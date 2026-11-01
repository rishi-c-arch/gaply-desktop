//! **The one-way ANOVA in a project (Research OS, Phase 8G-2, decisions 329
//! and 330).** Through the commands' plain bodies, as their Tauri wrappers
//! call them, over a root and a workspace session the test makes:
//!
//! * an ANOVA with a level left out and all three post hoc tests, and one
//!   with every level and none, saved, the project closed and opened, each
//!   run again, the rerun `Identical`, and the report saying so in decision
//!   189's words, with the levels' and the tests' lines;
//! * the stored spec names the levels the run included, ascending, so the
//!   rerun includes the same levels: its result has the same groups.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::runs::RunKind;
use gaply_core::research_project::RerunComparison;
use gaply_core::stats_anova::PostHoc;

use project_support::*;

/// A score and a group of four levels, a to d.
const DATA: &[u8] = b"score,arm\n\
1.5,a\n2.25,a\n3.75,a\n2.5,a\n\
4.5,b\n6.125,b\n7,b\n5.25,b\n\
9.5,c\n12.75,c\n8,c\n15.5,c\n\
3,d\n3.5,d\n4.25,d\n2.75,d\n";

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

/// The levels a stored result's groups name, in order.
fn group_levels(w: &World, id: &str, n: u64) -> Vec<u64> {
    let back = pc::run_result(&w.root(), &w.ws, id, n).expect("read back");
    let bundle: serde_json::Value =
        serde_json::from_str(back.bundle_json.as_deref().expect("a result")).unwrap();
    bundle["primaryOutputs"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["level"].as_u64().unwrap())
        .collect()
}

const THREE: [PostHoc; 3] = [PostHoc::Tukey, PostHoc::GamesHowell, PostHoc::Bonferroni];

#[test]
fn an_anova_is_saved_run_again_identical_with_its_levels_and_reported() {
    let w = World::new("8g2-project");
    let slot = w.open("anova.csv", DATA);
    // Level b (index 1) left out: the screen sends the ticked indexes,
    // ascending (decision 329).
    let left_out = w.anova(slot, 1, 2, Some(vec![0, 2, 3]), THREE.to_vec());
    let every = w.anova(slot, 1, 2, None, vec![]);
    let id = pc::save_run(&w.root(), &w.ws, left_out, new_project("ANOVA"))
        .expect("the ANOVA saves")
        .project_id;
    pc::save_run(&w.root(), &w.ws, every, into_open(&id)).expect("the second saves");
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    let a = pc::rerun(&w.root(), &w.ws, 1).expect("the first rerun starts");
    assert_eq!(a.kind, RunKind::Anova);
    let ra = settled(&w);
    assert_eq!(
        (ra.status, ra.stored_run_id),
        (RerunProgress::Saved, Some(3))
    );
    let b = pc::rerun(&w.root(), &w.ws, 2).expect("the second rerun starts");
    assert_eq!(b.kind, RunKind::Anova);
    let rb = settled(&w);
    assert_eq!(
        (rb.status, rb.stored_run_id),
        (RerunProgress::Saved, Some(4))
    );
    assert_eq!(comparison(&w, &id, 3), Some(RerunComparison::Identical));
    assert_eq!(comparison(&w, &id, 4), Some(RerunComparison::Identical));

    // The rerun included the levels the run did, and no other.
    assert_eq!(group_levels(&w, &id, 1), [0, 2, 3]);
    assert_eq!(group_levels(&w, &id, 3), [0, 2, 3]);
    assert_eq!(group_levels(&w, &id, 4), [0, 1, 2, 3]);

    let r = String::from_utf8(pc::report(&w.root(), &w.ws, &id).expect("the report").bytes)
        .expect("UTF-8");
    for n in [1, 2] {
        let sentence =
            format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    for line in [
        "- Column 1: outcome",
        "- Column 2: group",
        "- Levels included: indexes 0, 2, 3, in code-point order",
        "- Post hoc tests: tukey, games_howell, bonferroni",
        "- Levels included: every level of the group, in code-point order",
        "- Post hoc tests: none",
    ] {
        assert!(r.contains(line), "{line} missing:\n{r}");
    }
    assert_eq!(
        r.matches("- Levels included: indexes 0, 2, 3, in code-point order")
            .count(),
        2,
        "run 1 and its rerun"
    );
}
