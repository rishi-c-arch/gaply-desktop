//! **A stored run says whether it used a recipe (Research OS, Phase 8T-2b,
//! decisions 516 and 517).** `project_runs`' `StoredRunView` gains `prepared`,
//! serialised only when true, read from the graph `runs()` already loads: a run
//! with a `used` edge of role `recipe` is prepared, and no other is. So the
//! screen can say, before a rerun, that the scores are made again from the
//! recipe kept in the project.
//!
//! * On the wire: `"prepared": true` on a prepared run, and no key at all on a
//!   run of a file's own table, through the command's plain body over a project
//!   the test makes.
//! * `projectRuns.json`, which `tests/project_commands.rs` pins byte for byte
//!   through its two literals, is the file 8T-2a left: its bytes by SHA-256.

mod project_support;

use sha2::{Digest, Sha256};

use app_lib::project_commands::{self as pc, SaveTarget};
use app_lib::stats_desc_commands as desc;
use app_lib::stats_prep_commands as prep;
use gaply_core::contracts::{CapabilityId, OperationId, RunState};
use gaply_core::stats_desc::{self, Alias, ColumnRequest, DescribeParams, TreatAs};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};

use project_support::*;

/// Three items and a free column; the score is column 5.
const TABLE: &[u8] = b"q1,q2,q3,age\n4,2,5,31\n5,1,4,44\n2,4,2,29\n3,3,3,52\n1,5,2,37\n";

fn alias(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

/// The mean of the three items, its column 5.
fn mean_of_three() -> Recipe {
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: 4,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: alias(5),
            name: "Average".to_string(),
            items: (1..=3).map(alias).collect(),
            reversed: Vec::new(),
            scale: None,
            statistic: Some(Statistic::Mean),
            rule: Some(MissingRule::EveryItem),
            missing_codes: Vec::new(),
        })],
    }
}

/// A summary of `column` on `slot`, its spec carrying the slot's preparation as a
/// screen builds it, run to its end.
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

#[test]
fn a_stored_run_is_prepared_on_the_wire_only_when_it_used_a_recipe() {
    let w = World::new("stored-run-prepared");

    // Stored run 1: a summary of the file's own table.
    let plain = w.open("plain.csv", TABLE);
    let first = summary_of(&w, plain, 4);
    let saved = pc::save_run(
        &w.root(),
        &w.ws,
        first,
        SaveTarget::NewProject {
            name: "HT8T2".to_string(),
        },
    )
    .expect("it saves");
    let id = saved.project_id;

    // Stored run 2: a summary of the score, on the prepared slot.
    let prepared = w.open("items.csv", TABLE);
    let e = w.engine();
    let made = prep::make(&e.runs, &e.datasets, prepared, &mean_of_three(), 1_000_000)
        .unwrap_or_else(|_| panic!("the make"));
    assert!(made.dataset.is_some(), "{:?}", made.errors);
    let second = summary_of(&w, prepared, 5);
    pc::save_run(
        &w.root(),
        &w.ws,
        second,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .expect("it saves");

    let runs = pc::runs(&w.root(), &w.ws, &id).expect("the runs");
    assert_eq!(runs.len(), 2, "two stored runs");
    let wire = serde_json::to_value(&runs).unwrap();
    let rows = wire.as_array().unwrap();
    let by_id = |n: u64| {
        rows.iter()
            .find(|r| r["storedRunId"] == n)
            .unwrap_or_else(|| panic!("stored run {n}"))
    };
    assert!(
        by_id(1).get("prepared").is_none(),
        "a run of the file's own table sends no prepared key: {}",
        by_id(1)
    );
    assert_eq!(
        by_id(2).get("prepared"),
        Some(&serde_json::Value::Bool(true)),
        "a run that used a recipe sends prepared: true: {}",
        by_id(2)
    );
}

#[test]
fn project_runs_fixture_is_the_file_8t2a_left() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/projects/projectRuns.json");
    let bytes = std::fs::read(&p).expect("projectRuns.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "2ca605597ef89034b8593c6986abd13970c5ffa6d3844c4ca5cf98bc807627d5",
        "projectRuns.json changed: prepared must stay off the wire when false"
    );
    assert!(!String::from_utf8(bytes).unwrap().contains("prepared"));
}

/// The method's page, its large-samples lines and D309 (decision 518): the
/// stub's stale lines gone, the capability Beta, and the decision record the
/// field above cites.
#[test]
fn the_scale_score_page_is_whole_and_d309_is_recorded() {
    let docs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs");
    let read =
        |p: &str| std::fs::read_to_string(docs.join(p)).unwrap_or_else(|e| panic!("docs/{p}: {e}"));
    let page = read("methods/scale_scores.md");
    for stale in [
        "**Planned**",
        "This is a stub",
        "What does not exist yet",
        "a command, a project's copy of the recipe, a save path",
    ] {
        assert!(
            !page.contains(stale),
            "scale_scores.md still says {stale:?}"
        );
    }
    assert!(
        page.contains("**Beta**"),
        "scale_scores.md does not say Beta"
    );
    let sections = page.lines().filter(|l| l.starts_with("## ")).count();
    assert_eq!(
        sections, 10,
        "scale_scores.md has {sections} sections, not alpha's ten"
    );
    for needed in [
        "prepared_data.csv",
        "This file holds your data as Gaply read it, with your scores added.",
        "https://www.ibm.com/docs/en/spss-statistics/31.0.0?topic=expressions-statistical-functions",
        "Gaply does not rescale it",
    ] {
        assert!(page.contains(needed), "scale_scores.md lacks {needed:?}");
    }
    assert!(
        read("methods/large_samples.md").contains("to be measured by realapp_8t2b.md"),
        "large_samples.md lacks the export's route lines"
    );
    assert!(
        read("AI_ENGINE_PLAN.md").contains("\n### D309 — "),
        "no ### D309 in docs/AI_ENGINE_PLAN.md"
    );
}
