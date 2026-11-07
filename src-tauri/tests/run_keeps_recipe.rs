//! **A run keeps its recipe (Research OS, Phase 8T-1a, decision 491; ADR-012
//! question 9).** Through the commands' plain bodies, over a root and a
//! workspace session the test makes. A run on a prepared slot keeps the
//! recipe's canonical bytes in its record, so a Save writes the recipe the run
//! ran on, whatever the slot holds by then:
//!
//! * a second score made after the run (score A, a t-test on A, then A and B):
//!   the t-test saves with A's recipe;
//! * the slot's recipe cleared after the run: it saves with its recipe still.
//!
//! In each, the recipe's bytes on disk are the run's own recipe's, and they
//! hash to the `recipeHash` the run's spec recorded. Synthetic data only.

mod project_support;

use rusqlite::{Connection, OpenFlags};

use app_lib::project_commands::{self as pc, SaveTarget};
use app_lib::stats_prep_commands as prep;
use app_lib::stats_ttest_commands as ttest;
use gaply_core::contracts::{CapabilityId, OperationId, RunState};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};
use gaply_core::stats_ttest::{self, TTestParams, Variant};

use project_support::*;

/// Four items on 1 to 5 and a two-level group, column 5.
const TABLE: &[u8] = b"q1,q2,q3,q4,arm\n\
4,2,5,1,a\n5,1,4,2,a\n2,4,2,4,a\n3,3,3,3,a\n1,5,2,5,a\n\
4,2,4,2,b\n5,2,5,1,b\n2,3,3,3,b\n2,4,1,4,b\n3,3,4,2,b\n";

fn alias(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn score(at: u32, name: &str, items: &[u32]) -> RecipeStep {
    RecipeStep::ScaleScore(ScaleScoreStep {
        alias: alias(at),
        name: name.to_string(),
        items: items.iter().map(|i| alias(*i)).collect(),
        reversed: Vec::new(),
        scale: None,
        statistic: Some(Statistic::Mean),
        rule: Some(MissingRule::EveryItem),
        missing_codes: Vec::new(),
    })
}

fn recipe(steps: Vec<RecipeStep>) -> Recipe {
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: 5,
        steps,
    }
}

/// Score A, column 6: the mean of the first two items.
fn score_a() -> Recipe {
    recipe(vec![score(6, "score a", &[1, 2])])
}

/// Score A, then score B, column 7: the mean of the last two items.
fn scores_a_and_b() -> Recipe {
    recipe(vec![
        score(6, "score a", &[1, 2]),
        score(7, "score b", &[3, 4]),
    ])
}

fn make(w: &World, slot: u64, recipe: &Recipe) {
    let e = w.engine();
    let v = prep::make(&e.runs, &e.datasets, slot, recipe, 1_000_000)
        .map_err(|_| "the make")
        .unwrap();
    assert!(v.dataset.is_some(), "{:?}", v.errors);
}

/// A t-test of score A by the group, its spec as a screen builds one on this
/// slot now: (the run, the recipe hash its spec recorded).
fn ttest_on_a(w: &World, slot: u64) -> (u64, String) {
    let e = w.engine();
    let held = e.datasets.get(slot).expect("the slot");
    let mut spec = stats_ttest::spec(
        OperationId(1),
        held.dataset(),
        TTestParams {
            outcome: alias(6),
            group: alias(5),
            levels: None,
            variant: Some(Variant::Welch),
            missing_codes: Vec::new(),
        },
    );
    spec.preparation = held.preparation();
    let recorded = spec
        .preparation
        .as_ref()
        .expect("a prepared slot's spec names its preparation")
        .recipe_hash
        .as_hex()
        .to_string();
    let id = ttest::run(&e.runs, &e.datasets, slot, spec)
        .map(|v| v.run_id)
        .unwrap_or_else(|_| panic!("the t-test starts"));
    assert_eq!(w.wait(id), RunState::Succeeded);
    (id, recorded)
}

/// The project's one recipe artifact: its hash and its bytes on disk.
fn stored_recipe(w: &World, project_id: &str) -> (String, Vec<u8>) {
    let dir = w.root().join("projects").join(project_id);
    let c = Connection::open_with_flags(dir.join("project.db"), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap();
    let mut st = c
        .prepare("SELECT hash FROM artifacts WHERE kind = 'recipe'")
        .unwrap();
    let hashes: Vec<String> = st
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(hashes.len(), 1, "{hashes:?}");
    let bytes = std::fs::read(dir.join("artifacts").join(&hashes[0])).unwrap();
    (hashes[0].clone(), bytes)
}

fn new_project(name: &str) -> SaveTarget {
    SaveTarget::NewProject {
        name: name.to_string(),
    }
}

/// Its bytes are A's, never the slot's A and B, and they hash to the spec's.
fn assert_the_runs_own_recipe(w: &World, project_id: &str, recorded: &str) {
    let (hash, bytes) = stored_recipe(w, project_id);
    assert_eq!(bytes, stats_prep::recipe_bytes(&score_a()), "A's recipe");
    assert_eq!(hash, recorded, "the spec's recipeHash");
    assert_eq!(
        gaply_core::contracts::ContentHash::of_bytes(&bytes).as_hex(),
        recorded,
        "the bytes hash to the spec's recipeHash"
    );
}

#[test]
fn a_run_on_score_a_saves_with_as_recipe_after_score_b_is_made() {
    let w = World::new("8t1a-second-score");
    let slot = w.open("items.csv", TABLE);
    make(&w, slot, &score_a());
    let (run, recorded) = ttest_on_a(&w, slot);
    make(&w, slot, &scores_a_and_b());
    let saved = pc::save_run(&w.root(), &w.ws, run, new_project("Second score"))
        .expect("the t-test on A saves after B is made");
    assert_the_runs_own_recipe(&w, &saved.project_id, &recorded);
    assert_ne!(
        stats_prep::recipe_bytes(&score_a()),
        stats_prep::recipe_bytes(&scores_a_and_b()),
        "the slot now holds another recipe"
    );
}

#[test]
fn a_run_on_score_a_saves_with_as_recipe_after_a_clear() {
    let w = World::new("8t1a-after-clear");
    let slot = w.open("items.csv", TABLE);
    make(&w, slot, &score_a());
    let (run, recorded) = ttest_on_a(&w, slot);
    let e = w.engine();
    prep::clear(&e.runs, &e.datasets, slot)
        .map_err(|_| "clears")
        .unwrap();
    let saved = pc::save_run(&w.root(), &w.ws, run, new_project("After a clear"))
        .expect("the t-test on A saves after a clear");
    assert_the_runs_own_recipe(&w, &saved.project_id, &recorded);
}
