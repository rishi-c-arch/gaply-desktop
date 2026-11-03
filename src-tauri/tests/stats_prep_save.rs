//! **A Save of a run on a prepared slot (Research OS, Phase 8M-3, decision 425;
//! Phase 8M-4, decision 433).** Through the commands' plain bodies, over a root
//! and a workspace session the test makes. A run on a prepared table saves with
//! its recipe beside its data file and its spec. Once the recipe is cleared it
//! is refused as `project.data_mismatch`. A run made on the original before a
//! recipe still saves, as the slot's `SourceFile` is the same, with no recipe.

mod project_support;

use std::collections::BTreeMap;

use rusqlite::{Connection, OpenFlags};

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
fn a_save_of_a_run_on_a_prepared_slot_keeps_its_recipe_beside_its_data_and_its_spec() {
    let w = World::new("8m3-save-prepared");
    let slot = w.open("items.csv", DATA);
    make(&w, slot);
    let run = alpha(&w, slot, &[1, 2, 4]);
    let saved = pc::save_run(&w.root(), &w.ws, run, target("Prepared")).expect("it saves");

    let dir = w.root().join("projects").join(&saved.project_id);
    let c = Connection::open_with_flags(dir.join("project.db"), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap();
    let rows = |sql: &str| -> Vec<(String, String)> {
        let mut st = c.prepare(sql).unwrap();
        let out = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        out
    };
    let by_kind: BTreeMap<String, String> = rows("SELECT kind, hash FROM artifacts")
        .into_iter()
        .collect();
    assert_eq!(
        by_kind.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "data_file",
            "operation_spec",
            "recipe",
            "result_bundle",
            "run_manifest"
        ]
    );
    let bytes = stats_prep::recipe_bytes(&recipe());
    let recipe_hash = stats_prep::recipe_hash(&recipe());
    assert_eq!(by_kind["recipe"], recipe_hash);
    assert_eq!(
        std::fs::read(dir.join("artifacts").join(&recipe_hash)).unwrap(),
        bytes,
        "stats_prep's canonical bytes, under their own hash"
    );
    assert_eq!(
        rows("SELECT format, hash FROM artifacts WHERE kind = 'recipe'")[0].0,
        "json"
    );
    let used: BTreeMap<String, String> = rows("SELECT role, hash FROM used WHERE run_id = 1")
        .into_iter()
        .collect();
    assert_eq!(
        used.keys().map(String::as_str).collect::<Vec<_>>(),
        ["data", "recipe", "spec"]
    );
    assert_eq!(used["recipe"], recipe_hash);
    assert_eq!(used["data"], by_kind["data_file"]);
    assert_eq!(used["spec"], by_kind["operation_spec"]);
    let generated: Vec<String> = rows("SELECT hash, hash FROM generated WHERE run_id = 1")
        .into_iter()
        .map(|(h, _)| h)
        .collect();
    assert_eq!(generated.len(), 2, "the bundle and the manifest");
    assert!(generated.contains(&by_kind["result_bundle"]));
    assert!(generated.contains(&by_kind["run_manifest"]));
    assert!(
        rows("SELECT hash, from_hash FROM derived").is_empty(),
        "the derived table stays unused"
    );

    // Reading it back returns nothing of the recipe.
    let back = pc::run_result(&w.root(), &w.ws, &saved.project_id, 1).expect("it reads back");
    let text = serde_json::to_string(&back).unwrap();
    assert!(!text.contains("wellbeing"), "{text}");
    assert_eq!(back.data_file.sha256, by_kind["data_file"]);
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
    let dir = w.root().join("projects").join(&saved.project_id);
    let recipes: i64 =
        Connection::open_with_flags(dir.join("project.db"), OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM artifacts WHERE kind = 'recipe'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(recipes, 0, "this run read no recipe");
}
