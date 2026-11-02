//! **Cronbach's alpha in a project (Research OS, Phase 8I-2, decision
//! 382).** Through the commands' plain bodies, as their Tauri wrappers call
//! them, over a root and a workspace session the test makes:
//!
//! * alpha with two items reverse-scored on the scale's ends and a ticked
//!   missing code, and alpha with nothing reversed, saved, the project closed
//!   and opened, each run again, the rerun `Identical`, and the report saying
//!   so in decision 189's words, with the items', reverse-scored items' and
//!   scale's lines;
//! * the stored spec keeps the reversed items and the scale, so the rerun
//!   reverses what the run did: its items' flags and means are the run's.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::runs::RunKind;
use gaply_core::research_project::RerunComparison;

use project_support::*;

/// Four five-point items, the second and fourth reverse-worded, with cells
/// missing in different rows and one -9.
const DATA: &[u8] = b"calm,tense,rested,worried\n\
4,2,5,1\n5,1,,2\n2,4,2,4\n3,-9,3,3\n\
1,5,2,5\n4,2,4,2\n5,2,5,1\n,3,3,3\n\
2,4,1,4\n3,3,4,2\n";

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

/// Each item of a stored result: its alias, whether it was reversed, and
/// its mean.
fn items(w: &World, id: &str, n: u64) -> Vec<(String, bool, serde_json::Value)> {
    let back = pc::run_result(&w.root(), &w.ws, id, n).expect("read back");
    let bundle: serde_json::Value =
        serde_json::from_str(back.bundle_json.as_deref().expect("a result")).unwrap();
    bundle["primaryOutputs"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["item"].as_str().unwrap().to_string(),
                i["reversed"].as_bool().unwrap(),
                i["mean"].clone(),
            )
        })
        .collect()
}

#[test]
fn cronbach_alpha_is_saved_run_again_identical_and_reported() {
    let w = World::new("8i2-project");
    let slot = w.open("alpha.csv", DATA);
    let reversed = w.alpha(slot, &[1, 2, 3, 4], &[2, 4], Some((1.0, 5.0)), vec![-9.0]);
    let plain = w.alpha(slot, &[1, 3], &[], None, vec![]);
    let id = pc::save_run(&w.root(), &w.ws, reversed, new_project("Alpha"))
        .expect("alpha saves")
        .project_id;
    pc::save_run(&w.root(), &w.ws, plain, into_open(&id)).expect("the second saves");
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    let a = pc::rerun(&w.root(), &w.ws, 1).expect("the first rerun starts");
    assert_eq!(a.kind, RunKind::CronbachAlpha);
    let ra = settled(&w);
    assert_eq!(
        (ra.status, ra.stored_run_id),
        (RerunProgress::Saved, Some(3))
    );
    let b = pc::rerun(&w.root(), &w.ws, 2).expect("the second rerun starts");
    assert_eq!(b.kind, RunKind::CronbachAlpha);
    let rb = settled(&w);
    assert_eq!(
        (rb.status, rb.stored_run_id),
        (RerunProgress::Saved, Some(4))
    );
    assert_eq!(comparison(&w, &id, 3), Some(RerunComparison::Identical));
    assert_eq!(comparison(&w, &id, 4), Some(RerunComparison::Identical));

    // The rerun reversed the items the run did, and nothing else.
    let flags = |v: Vec<(String, bool, serde_json::Value)>| -> Vec<(String, bool)> {
        v.into_iter().map(|(a, r, _)| (a, r)).collect()
    };
    let want = vec![
        ("C1".to_string(), false),
        ("C2".to_string(), true),
        ("C3".to_string(), false),
        ("C4".to_string(), true),
    ];
    assert_eq!(flags(items(&w, &id, 1)), want);
    assert_eq!(flags(items(&w, &id, 3)), want);
    assert_eq!(items(&w, &id, 3), items(&w, &id, 1), "the same means");
    assert!(flags(items(&w, &id, 4)).iter().all(|(_, r)| !r));

    let r = String::from_utf8(pc::report(&w.root(), &w.ws, &id).expect("the report").bytes)
        .expect("UTF-8");
    for n in [1, 2] {
        let sentence =
            format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    for line in [
        "- Items: columns 1, 2, 3, 4, by position",
        "- Reverse-scored: columns 2, 4, by position",
        "- Scale ends: 1 to 5",
        "- Items: columns 1, 3, by position",
        "- Reverse-scored: none",
        "- Scale ends: not given, as nothing is reverse-scored",
        "- Missing-value codes: 1 (their values are in the settings fingerprint)",
    ] {
        assert!(r.contains(line), "{line} missing:\n{r}");
    }
    assert_eq!(
        r.matches("- Reverse-scored: columns 2, 4, by position")
            .count(),
        2,
        "run 1 and its rerun"
    );
    // No header reaches the report.
    for header in ["calm", "tense", "rested", "worried"] {
        assert!(!r.contains(header), "{header} in the report:\n{r}");
    }
}
