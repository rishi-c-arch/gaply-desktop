//! **Crosstabs in a project (Research OS, Phase 8X-2, decision 590).** Through
//! the commands' plain bodies, as their Tauri wrappers call them, over a root and
//! a workspace session the test makes, on `project_anova.rs`'s pattern:
//!
//! * a 2 x 2 crosstab with a missing code and a 3 x 4 one with none, saved, the
//!   project closed and opened, each run read back, each run again, the rerun
//!   `Identical`, and the report saying so in decision 189's words, with each
//!   run's two columns by position and role;
//! * a crosstab on two file columns of a prepared slot (a slot with a scale
//!   score made), its spec carrying the slot's preparation as a screen's does
//!   (decision 490), saved and run again `Identical`. Crosstabs refuses a score
//!   as a column, so it does not join `prep_project.rs`'s NINE.
//!
//! Every file is synthetic and in a folder the test creates and removes.

mod project_support;

use std::time::{Duration, Instant};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::runs::RunKind;
use app_lib::stats_prep_commands as prep;
use gaply_core::contracts::{CapabilityId, RunState};
use gaply_core::research_project::RerunComparison;
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};

use project_support::*;

/// Two items, a two-level arm, a two-level sex with a -99 code, and a
/// three-level site; column 6 holds four levels of text.
const DATA: &[u8] = b"q1,q2,arm,sex,site,dose\n\
4,2,a,f,x,low\n5,1,a,m,x,mid\n2,4,a,f,y,high\n3,3,a,-99,y,top\n\
1,5,a,m,z,low\n4,2,a,f,z,mid\n5,2,b,m,x,high\n2,3,b,f,x,top\n\
2,4,b,m,y,low\n3,3,b,f,y,mid\n1,4,b,m,z,high\n5,1,b,-99,z,top\n\
3,2,a,f,x,low\n4,4,b,m,y,mid\n2,5,a,m,z,high\n1,3,b,f,x,top\n";

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

/// A stored result's table: its level counts and its cell count.
fn shape(w: &World, id: &str, n: u64) -> (usize, usize, u64) {
    let back = pc::run_result(&w.root(), &w.ws, id, n).expect("read back");
    let bundle: serde_json::Value =
        serde_json::from_str(back.bundle_json.as_deref().expect("a result")).unwrap();
    let o = &bundle["primaryOutputs"];
    (
        o["rowLevels"].as_array().unwrap().len(),
        o["columnLevels"].as_array().unwrap().len(),
        o["cellCount"].as_u64().unwrap(),
    )
}

fn alias(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

#[test]
fn a_crosstab_is_saved_read_back_run_again_identical_and_reported() {
    let w = World::new("8x2-project");
    let slot = w.open("xtab.csv", DATA);
    // arm by sex, -99 ticked: a 2 x 2 table.
    let two = w.xtab(slot, 3, 4, vec![-99.0]);
    // site by dose: a 3 x 4 table.
    let larger = w.xtab(slot, 5, 6, vec![]);
    for run in [two, larger] {
        assert_eq!(w.wait_new_kind(run), RunState::Succeeded);
    }
    let id = pc::save_run(&w.root(), &w.ws, two, new_project("Crosstabs"))
        .expect("the 2 x 2 saves")
        .project_id;
    pc::save_run(&w.root(), &w.ws, larger, into_open(&id)).expect("the 3 x 4 saves");
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");

    // Read back: each stored result is the table it was.
    assert_eq!(shape(&w, &id, 1), (2, 2, 4));
    assert_eq!(shape(&w, &id, 2), (3, 4, 12));

    let a = pc::rerun(&w.root(), &w.ws, 1).expect("the first rerun starts");
    assert_eq!(a.kind, RunKind::Crosstabs);
    let ra = settled(&w);
    assert_eq!(
        (ra.status, ra.stored_run_id),
        (RerunProgress::Saved, Some(3))
    );
    let b = pc::rerun(&w.root(), &w.ws, 2).expect("the second rerun starts");
    assert_eq!(b.kind, RunKind::Crosstabs);
    let rb = settled(&w);
    assert_eq!(
        (rb.status, rb.stored_run_id),
        (RerunProgress::Saved, Some(4))
    );
    assert_eq!(comparison(&w, &id, 3), Some(RerunComparison::Identical));
    assert_eq!(comparison(&w, &id, 4), Some(RerunComparison::Identical));
    assert_eq!(shape(&w, &id, 3), (2, 2, 4));
    assert_eq!(shape(&w, &id, 4), (3, 4, 12));

    // The report, which refused a saved crosstab before this step (D313).
    let r = String::from_utf8(pc::report(&w.root(), &w.ws, &id).expect("the report").bytes)
        .expect("UTF-8");
    for n in [1, 2] {
        let sentence =
            format!("Repeats run {n}. Its result is identical, byte for byte, to run {n}'s.");
        assert!(r.contains(&sentence), "the report repeats run {n}:\n{r}");
    }
    for (line, times) in [
        ("- Column 3: table_rows", 2),
        ("- Column 4: table_columns", 2),
        ("- Column 5: table_rows", 2),
        ("- Column 6: table_columns", 2),
        (
            "- Missing-value codes: 1 (their values are in the settings fingerprint)",
            2,
        ),
    ] {
        assert_eq!(r.matches(line).count(), times, "{line}, run and rerun:\n{r}");
    }
    // No level label and no header reaches the report (a rough check; the
    // privacy matrix checks labels with sentinels).
    for text in ["arm", "sex", "site", "dose", "low", "high", "top"] {
        assert!(
            !r.contains(&format!(" {text}\n")) && !r.contains(&format!(": {text}")),
            "{text} in the report"
        );
    }
}

/// The mean of q1 and q2, its score column 7.
fn mean_recipe() -> Recipe {
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: 6,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: alias(7),
            name: "a score of mine".to_string(),
            items: vec![alias(1), alias(2)],
            reversed: Vec::new(),
            scale: None,
            statistic: Some(Statistic::Mean),
            rule: Some(MissingRule::EveryItem),
            missing_codes: Vec::new(),
        })],
    }
}

#[test]
fn a_crosstab_on_a_prepared_slot_carries_its_preparation_and_reruns_identical() {
    let w = World::new("8x2-prepared");
    let slot = w.open("xtab.csv", DATA);
    {
        let e = w.engine();
        let v = prep::make(&e.runs, &e.datasets, slot, &mean_recipe(), 1_000_000)
            .map_err(|_| "the make")
            .unwrap();
        assert!(v.dataset.is_some(), "{:?}", v.errors);
        assert!(
            e.datasets.get(slot).unwrap().preparation().is_some(),
            "the slot is prepared"
        );
    }
    // Two file columns of the prepared slot, its preparation copied.
    let run = w.xtab(slot, 3, 5, vec![]);
    assert_eq!(w.wait_new_kind(run), RunState::Succeeded);
    let id = pc::save_run(&w.root(), &w.ws, run, new_project("Prepared crosstab"))
        .expect("saves")
        .project_id;
    let stored = pc::runs(&w.root(), &w.ws, &id).unwrap();
    assert_eq!(stored.len(), 1);
    pc::close(&w.root(), &w.ws).expect("closes");
    pc::open(&w.root(), &w.ws, &id).expect("opens");
    let a = pc::rerun(&w.root(), &w.ws, 1).expect("the rerun starts");
    assert_eq!(a.kind, RunKind::Crosstabs);
    let ra = settled(&w);
    assert_eq!(
        (ra.status, ra.stored_run_id),
        (RerunProgress::Saved, Some(2))
    );
    assert_eq!(comparison(&w, &id, 2), Some(RerunComparison::Identical));
    assert_eq!(shape(&w, &id, 2), (2, 3, 6));
    let r = String::from_utf8(pc::report(&w.root(), &w.ws, &id).expect("the report").bytes)
        .expect("UTF-8");
    assert_eq!(r.matches("- Column 3: table_rows").count(), 2, "{r}");
    assert_eq!(r.matches("- Column 5: table_columns").count(), 2, "{r}");
    assert!(!r.contains("a score of mine"), "the score's name in the report");
}
