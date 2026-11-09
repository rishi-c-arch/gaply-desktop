//! **A project's recipes, read for its page (Research OS, Phase 8U-3,
//! decisions 552 and 553).** `project_recipes` gives the open project's
//! recipes from the loaded graph's `used` edges of role `recipe`: each
//! recipe's users, ascending, reruns included; the file's own column count;
//! and each score by its position, with the name the user typed, its
//! statistic and how many items it has. A recipe whose file cannot be read
//! is still listed, its file's fields `null`, and the others stay whole. It
//! is refused as `project_runs` is, and loads the graph once.
//!
//! Every project here is made through the commands' plain bodies, in the
//! test's own folder, from a synthetic table.

mod project_support;

use std::time::{Duration, Instant};

use rusqlite::Connection;

use app_lib::project_commands::{self as pc, RecipeView, RerunProgress, RerunView, SaveTarget};
use app_lib::stats_desc_commands as desc;
use app_lib::stats_prep_commands as prep;
use gaply_core::contracts::{CapabilityId, OperationId, RunState};
use gaply_core::stats_desc::{self, Alias, ColumnRequest, DescribeParams, TreatAs};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};

use project_support::*;

/// Three items and a free column; scores are columns 5 and on.
const TABLE: &[u8] = b"q1,q2,q3,age\n4,2,5,31\n5,1,4,44\n2,4,2,29\n3,3,3,52\n1,5,2,37\n";

fn alias(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

/// A score of items 1 to `k` at `position`.
fn score(position: u32, name: &str, statistic: Statistic, k: u32) -> RecipeStep {
    RecipeStep::ScaleScore(ScaleScoreStep {
        alias: alias(position),
        name: name.to_string(),
        items: (1..=k).map(alias).collect(),
        reversed: Vec::new(),
        scale: None,
        statistic: Some(statistic),
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
        source_columns: 4,
        steps,
    }
}

/// A summary of `column` on `slot`, its spec carrying the slot's preparation.
fn summary_of(w: &World, slot: u64, column: u32) -> u64 {
    let e = w.engine();
    let held = e.datasets.get(slot).expect("the slot");
    let params = DescribeParams {
        columns: vec![ColumnRequest {
            alias: alias(column),
            treat_as: TreatAs::Numeric,
        }],
        missing_codes: Vec::new(),
    };
    let mut spec = stats_desc::spec(OperationId(1), held.dataset(), params);
    spec.preparation = held.preparation();
    let id = desc::run(&e.runs, &e.datasets, slot, spec)
        .unwrap_or_else(|_| panic!("the run starts"))
        .run_id;
    assert_eq!(w.wait(id), RunState::Succeeded);
    id
}

/// Saves `run` into the open project, or a new one when there is none; the
/// project's id and the stored run's.
fn save(w: &World, run: u64, into: Option<&str>) -> (String, u64) {
    let target = match into {
        Some(id) => SaveTarget::OpenProject {
            project_id: id.to_string(),
        },
        None => SaveTarget::NewProject {
            name: "Recipes".to_string(),
        },
    };
    let saved = pc::save_run(&w.root(), &w.ws, run, target).expect("it saves");
    (saved.project_id, saved.stored_run_id)
}

/// A summary of a file's own table, saved; the project's id.
fn with_a_plain_run(w: &World) -> String {
    let slot = w.open("plain.csv", TABLE);
    let run = summary_of(w, slot, 4);
    save(w, run, None).0
}

/// A summary of the last score `r` makes, on a file of its own, saved into
/// `id`; the stored run's id.
fn save_prepared(w: &World, id: &str, file: &str, r: &Recipe) -> u64 {
    let slot = w.open(file, TABLE);
    let e = w.engine();
    let made = prep::make(&e.runs, &e.datasets, slot, r, 1_000_000)
        .unwrap_or_else(|_| panic!("the make"));
    assert!(made.dataset.is_some(), "{:?}", made.errors);
    let last = 4 + r.steps.len() as u32;
    let run = summary_of(w, slot, last);
    save(w, run, Some(id)).1
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
            start.elapsed() < Duration::from_secs(20),
            "the rerun never settled"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn read(w: &World, id: &str) -> Vec<RecipeView> {
    pc::recipes(&w.root(), &w.ws, id).unwrap_or_else(|e| panic!("project_recipes: {e}"))
}

fn wire(views: &[RecipeView]) -> serde_json::Value {
    serde_json::to_value(views).unwrap()
}

fn artifact_path(w: &World, id: &str, r: &Recipe) -> std::path::PathBuf {
    w.root()
        .join("projects")
        .join(id)
        .join("artifacts")
        .join(stats_prep::recipe_hash(r))
}

/* ---- what it lists ------------------------------------------------------------ */

#[test]
fn a_project_with_no_prepared_run_has_no_recipes() {
    let w = World::new("recipes-none");
    let id = with_a_plain_run(&w);
    assert_eq!(wire(&read(&w, &id)), serde_json::json!([]));
}

#[test]
fn a_recipe_lists_its_user_its_columns_and_each_score_by_position_with_its_name() {
    let w = World::new("recipes-one");
    let id = with_a_plain_run(&w);
    let two = recipe(vec![
        score(5, "Average", Statistic::Mean, 3),
        score(6, "Total", Statistic::Sum, 2),
    ]);
    let stored = save_prepared(&w, &id, "items.csv", &two);
    assert_eq!(stored, 2);
    assert_eq!(
        wire(&read(&w, &id)),
        serde_json::json!([{
            "usedBy": [2],
            "sourceColumns": 4,
            "scores": [
                { "position": 5, "name": "Average", "statistic": "mean", "items": 3 },
                { "position": 6, "name": "Total", "statistic": "sum", "items": 2 },
            ],
        }])
    );
}

#[test]
fn a_rerun_of_a_prepared_run_is_listed_among_its_recipes_users() {
    let w = World::new("recipes-rerun");
    let id = with_a_plain_run(&w);
    let r = recipe(vec![score(5, "Average", Statistic::Mean, 3)]);
    assert_eq!(save_prepared(&w, &id, "items.csv", &r), 2);
    pc::rerun(&w.root(), &w.ws, 2).expect("the rerun starts");
    assert_eq!(settled(&w).status, RerunProgress::Saved);
    let views = read(&w, &id);
    assert_eq!(views.len(), 1, "the rerun reuses the recipe by its hash");
    assert_eq!(views[0].used_by, vec![2, 3]);
}

#[test]
fn two_recipes_are_listed_in_the_order_of_their_first_users() {
    let w = World::new("recipes-two");
    let id = with_a_plain_run(&w);
    let a = recipe(vec![score(5, "Average", Statistic::Mean, 3)]);
    let b = recipe(vec![score(5, "Total", Statistic::Sum, 3)]);
    // Saved so the order of first use is the reverse of the hashes' order.
    let (first, second) = if stats_prep::recipe_hash(&a) > stats_prep::recipe_hash(&b) {
        (a, b)
    } else {
        (b, a)
    };
    assert_eq!(save_prepared(&w, &id, "first.csv", &first), 2);
    assert_eq!(save_prepared(&w, &id, "second.csv", &second), 3);
    let views = read(&w, &id);
    let names: Vec<(&str, &[u64])> = views
        .iter()
        .map(|v| (v.scores.as_ref().unwrap()[0].name.as_str(), v.used_by.as_slice()))
        .collect();
    let name = |r: &Recipe| match &r.steps[0] {
        RecipeStep::ScaleScore(s) => s.name.clone(),
    };
    assert_eq!(
        names,
        vec![(name(&first).as_str(), &[2u64][..]), (name(&second).as_str(), &[3u64][..])]
    );
}

#[test]
fn a_changed_recipe_file_and_a_missing_one_are_listed_with_null_fields_and_the_other_whole() {
    let w = World::new("recipes-damaged");
    let id = with_a_plain_run(&w);
    let changed = recipe(vec![score(5, "Changed", Statistic::Mean, 3)]);
    let gone = recipe(vec![score(5, "Gone", Statistic::Mean, 3)]);
    let whole = recipe(vec![score(5, "Whole", Statistic::Sum, 3)]);
    assert_eq!(save_prepared(&w, &id, "a.csv", &changed), 2);
    assert_eq!(save_prepared(&w, &id, "b.csv", &gone), 3);
    assert_eq!(save_prepared(&w, &id, "c.csv", &whole), 4);

    // The artifact files are read-only: write beside, then rename over.
    let p = artifact_path(&w, &id, &changed);
    let mut bytes = std::fs::read(&p).unwrap();
    bytes[0] ^= 1;
    let beside = p.with_extension("changed");
    std::fs::write(&beside, &bytes).unwrap();
    std::fs::rename(&beside, &p).unwrap();
    std::fs::remove_file(artifact_path(&w, &id, &gone)).unwrap();

    let want = serde_json::json!([
        { "usedBy": [2], "sourceColumns": null, "scores": null },
        { "usedBy": [3], "sourceColumns": null, "scores": null },
        {
            "usedBy": [4],
            "sourceColumns": 4,
            "scores": [{ "position": 5, "name": "Whole", "statistic": "sum", "items": 3 }],
        },
    ]);
    assert_eq!(wire(&read(&w, &id)), want, "the first read marks them damaged");
    assert_eq!(wire(&read(&w, &id)), want, "a row already damaged reads the same");
    // The runs read marks nothing new and still lists every run.
    assert_eq!(pc::runs(&w.root(), &w.ws, &id).unwrap().len(), 4);
}

#[test]
fn a_run_with_two_recipe_edges_is_listed_under_both() {
    let w = World::new("recipes-two-edges");
    let id = with_a_plain_run(&w);
    let a = recipe(vec![score(5, "Average", Statistic::Mean, 3)]);
    let b = recipe(vec![score(5, "Total", Statistic::Sum, 3)]);
    assert_eq!(save_prepared(&w, &id, "a.csv", &a), 2);
    assert_eq!(save_prepared(&w, &id, "b.csv", &b), 3);
    pc::close(&w.root(), &w.ws).unwrap();
    let db = w.root().join("projects").join(&id).join("project.db");
    Connection::open(db)
        .unwrap()
        .execute(
            "INSERT INTO used (run_id, hash, role) VALUES (2, ?1, 'recipe')",
            [stats_prep::recipe_hash(&b)],
        )
        .unwrap();
    pc::open(&w.root(), &w.ws, &id).unwrap();
    let views = read(&w, &id);
    assert_eq!(views.len(), 2);
    // Both are first used by run 2 now, so their order is the hashes'.
    let by_name = |n: &str| {
        views
            .iter()
            .find(|v| v.scores.as_ref().is_some_and(|s| s[0].name == n))
            .unwrap_or_else(|| panic!("{n}'s recipe"))
            .used_by
            .clone()
    };
    assert_eq!(by_name("Average"), vec![2], "a's");
    assert_eq!(by_name("Total"), vec![2, 3], "b's, with run 2's second edge");
}

/* ---- refused as runs is ------------------------------------------------------ */

#[test]
fn it_is_refused_as_project_runs_is() {
    let w = World::new("recipes-refused");
    let id = with_a_plain_run(&w);
    let r = w.root();
    let e = pc::recipes(&r, &w.ws, &"b".repeat(32)).unwrap_err();
    assert_eq!(code(&e), "project.not_open", "another project");
    let e = pc::recipes(&r, &w.ws, "../x").unwrap_err();
    assert_eq!(code(&e), "project.bad_project_id");
    pc::close(&r, &w.ws).unwrap();
    let e = pc::recipes(&r, &w.ws, &id).unwrap_err();
    assert_eq!(code(&e), "project.not_open", "none open");
    pc::open(&r, &w.ws, &id).unwrap();
    w.ws.deactivate().expect("Statistics is left");
    let e = pc::recipes(&r, &w.ws, &id).unwrap_err();
    assert_eq!(code(&e), "workspace.inactive", "Statistics not active");
}

#[test]
fn it_loads_the_provenance_graph_once() {
    use app_lib::project_store::{graph_loads_on_this_thread, reset_graph_loads_on_this_thread};
    let w = World::new("recipes-one-load");
    let id = with_a_plain_run(&w);
    let a = recipe(vec![score(5, "Average", Statistic::Mean, 3)]);
    let b = recipe(vec![score(5, "Total", Statistic::Sum, 3)]);
    save_prepared(&w, &id, "a.csv", &a);
    save_prepared(&w, &id, "b.csv", &b);
    reset_graph_loads_on_this_thread();
    assert_eq!(read(&w, &id).len(), 2);
    assert_eq!(graph_loads_on_this_thread(), 1, "project_recipes");
}

/* ---- the name, in memory only ------------------------------------------------- */

#[test]
fn a_recipe_view_debugs_as_counts_and_never_names_a_score() {
    let w = World::new("recipes-debug");
    let id = with_a_plain_run(&w);
    let r = recipe(vec![score(5, "NAME_SENTINEL_r55", Statistic::Mean, 3)]);
    save_prepared(&w, &id, "a.csv", &r);
    let views = read(&w, &id);
    assert!(
        wire(&views).to_string().contains("NAME_SENTINEL_r55"),
        "the response holds the name: the page's one surface for it"
    );
    let printed = format!("{views:?} {:?}", views[0].scores.as_ref().unwrap()[0]);
    assert!(printed.contains("ScoreView"), "{printed}");
    assert!(!printed.contains("NAME_SENTINEL_r55"), "{printed}");
}
