//! **The correlations in a project (Research OS, Phase 8H-2, decision
//! 357).** Through the commands' plain bodies, as their Tauri wrappers call
//! them, over a root and a workspace session the test makes:
//!
//! * correlations pairwise with all three coefficients and a ticked missing
//!   code, and listwise with Pearson only, saved, the project closed and
//!   opened, each run again, the rerun `Identical`, and the report saying so
//!   in decision 189's words, with the variables', coefficients' and missing
//!   handling's lines;
//! * the stored spec names the variables, coefficients and codes in the
//!   order the method requires, so the rerun computes the same pairs: its
//!   result has the same tables.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::runs::RunKind;
use gaply_core::research_project::RerunComparison;
use gaply_core::stats_corr::{Coefficient, MissingHandling};

use project_support::*;

/// Three numeric columns with cells missing in different rows and one -9.
const DATA: &[u8] = b"trust,loyalty,price\n\
1.5,2,9\n2.25,,8.5\n3,3.5,-9\n4.5,4,6.25\n\
,6,5\n6.75,5.5,4\n7,8,3.5\n8.5,7.25,2\n\
9,9.5,1.25\n10.25,-9,0.5\n";

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

/// The coefficients a stored result's tables name, in order.
fn tables(w: &World, id: &str, n: u64) -> Vec<String> {
    let back = pc::run_result(&w.root(), &w.ws, id, n).expect("read back");
    let bundle: serde_json::Value =
        serde_json::from_str(back.bundle_json.as_deref().expect("a result")).unwrap();
    bundle["primaryOutputs"]["coefficients"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["coefficient"].as_str().unwrap().to_string())
        .collect()
}

const THREE: [Coefficient; 3] = [
    Coefficient::Pearson,
    Coefficient::Spearman,
    Coefficient::Kendall,
];

#[test]
fn correlations_are_saved_run_again_identical_and_reported() {
    let w = World::new("8h2-project");
    let slot = w.open("corr.csv", DATA);
    let pairwise = w.corr(
        slot,
        &[1, 2, 3],
        THREE.to_vec(),
        MissingHandling::Pairwise,
        vec![-9.0],
    );
    let listwise = w.corr(
        slot,
        &[1, 3],
        vec![Coefficient::Pearson],
        MissingHandling::Listwise,
        vec![-9.0],
    );
    let id = pc::save_run(&w.root(), &w.ws, pairwise, new_project("Correlations"))
        .expect("the correlations save")
        .project_id;
    pc::save_run(&w.root(), &w.ws, listwise, into_open(&id)).expect("the second saves");
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    let a = pc::rerun(&w.root(), &w.ws, 1).expect("the first rerun starts");
    assert_eq!(a.kind, RunKind::Correlations);
    let ra = settled(&w);
    assert_eq!(
        (ra.status, ra.stored_run_id),
        (RerunProgress::Saved, Some(3))
    );
    let b = pc::rerun(&w.root(), &w.ws, 2).expect("the second rerun starts");
    assert_eq!(b.kind, RunKind::Correlations);
    let rb = settled(&w);
    assert_eq!(
        (rb.status, rb.stored_run_id),
        (RerunProgress::Saved, Some(4))
    );
    assert_eq!(comparison(&w, &id, 3), Some(RerunComparison::Identical));
    assert_eq!(comparison(&w, &id, 4), Some(RerunComparison::Identical));

    // The rerun computed the coefficients the run did, in its order.
    assert_eq!(tables(&w, &id, 1), ["pearson", "spearman", "kendall"]);
    assert_eq!(tables(&w, &id, 3), ["pearson", "spearman", "kendall"]);
    assert_eq!(tables(&w, &id, 4), ["pearson"]);

    let r = String::from_utf8(pc::report(&w.root(), &w.ws, &id).expect("the report").bytes)
        .expect("UTF-8");
    for n in [1, 2] {
        let sentence =
            format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    for line in [
        "- Variables: columns 1, 2, 3, by position",
        "- Coefficients: pearson, spearman, kendall",
        "- Missing values: pairwise, each pair uses the rows where both have a value",
        "- Variables: columns 1, 3, by position",
        "- Coefficients: pearson",
        "- Missing values: listwise, every pair uses the rows where every variable has a value",
        "- Missing-value codes: 1 (their values are in the settings fingerprint)",
    ] {
        assert!(r.contains(line), "{line} missing:\n{r}");
    }
    assert_eq!(
        r.matches("- Coefficients: pearson, spearman, kendall")
            .count(),
        2,
        "run 1 and its rerun"
    );
    // No header reaches the report.
    for header in ["trust", "loyalty", "price"] {
        assert!(!r.contains(header), "{header} in the report:\n{r}");
    }
}
