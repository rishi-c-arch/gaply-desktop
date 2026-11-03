//! **A Save of a run on a prepared slot, as it stands until 8M-4 (Research OS,
//! Phase 8M-3, decision 425).** Through the commands' plain bodies, over a root
//! and a workspace session the test makes. A run on a prepared table is refused
//! as `project.data_mismatch`, since the run's input is the source's while the
//! spec's is the prepared one (runs.rs's `queue`, project_commands.rs's
//! `data_of`). A run made on the original before a recipe still saves, as the
//! slot's `SourceFile` is the same. 8M-4 changes the first; these pin today's.

mod project_support;

use app_lib::project_commands::{self as pc, SaveTarget};
use app_lib::stats_prep_commands as prep;
use gaply_core::contracts::{CapabilityId, OperationId, RunId, RunState};
use gaply_core::stats_alpha::{self, AlphaParams};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};

use project_support::*;

const DATA: &[u8] = b"q1,q2,q3\n4,2,5\n5,1,4\n2,4,2\n3,3,3\n1,5,2\n4,2,4\n5,2,5\n2,3,3\n";

fn alias(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn recipe() -> Recipe {
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: 3,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: alias(4),
            name: "wellbeing".into(),
            items: vec![alias(1), alias(2)],
            reversed: Vec::new(),
            scale: None,
            statistic: Some(Statistic::Sum),
            rule: Some(MissingRule::EveryItem),
            missing_codes: Vec::new(),
        })],
    }
}

fn make(w: &World, slot: u64) {
    let e = w.engine();
    let v = prep::make(&e.runs, &e.datasets, slot, &recipe(), 1_000_000)
        .map_err(|_| "the make")
        .unwrap();
    assert!(v.dataset.is_some(), "{:?}", v.errors);
}

/// Alpha over `items`, its spec as a screen builds one on this slot now.
fn alpha(w: &World, slot: u64, items: &[u32]) -> u64 {
    let e = w.engine();
    let held = e.datasets.get(slot).expect("the slot");
    let mut spec = stats_alpha::spec(
        OperationId(5),
        held.dataset(),
        AlphaParams {
            items: items.iter().map(|i| alias(*i)).collect(),
            reversed: Vec::new(),
            scale: None,
            missing_codes: Vec::new(),
        },
    );
    spec.preparation = held.preparation();
    let id = app_lib::stats_alpha_commands::run(&e.runs, &e.datasets, slot, spec)
        .expect("the run starts")
        .run_id;
    assert_eq!(w.wait_new_kind(id), RunState::Succeeded);
    let _ = RunId(id);
    id
}

fn target(name: &str) -> SaveTarget {
    SaveTarget::NewProject {
        name: name.to_string(),
    }
}

#[test]
fn a_save_of_a_run_on_a_prepared_slot_is_refused_as_a_data_mismatch() {
    let w = World::new("8m3-save-prepared");
    let slot = w.open("items.csv", DATA);
    make(&w, slot);
    let run = alpha(&w, slot, &[1, 2, 4]);
    let e = pc::save_run(&w.root(), &w.ws, run, target("Prepared"))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(code(&e), "project.data_mismatch");
}

#[test]
fn a_save_of_a_prepared_run_after_a_clear_is_refused_too() {
    let w = World::new("8m3-save-cleared");
    let slot = w.open("items.csv", DATA);
    make(&w, slot);
    let run = alpha(&w, slot, &[1, 2, 4]);
    let e = w.engine();
    prep::clear(&e.runs, &e.datasets, slot)
        .map_err(|_| "clears")
        .unwrap();
    let refused = pc::save_run(&w.root(), &w.ws, run, target("Cleared"))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(code(&refused), "project.data_mismatch");
}

#[test]
fn a_run_made_on_the_original_before_a_recipe_still_saves() {
    let w = World::new("8m3-save-original");
    let slot = w.open("items.csv", DATA);
    let run = alpha(&w, slot, &[1, 2, 3]);
    make(&w, slot);
    let saved = pc::save_run(&w.root(), &w.ws, run, target("Original")).expect("it saves");
    assert!(!saved.project_id.is_empty());
}
