//! **Scale scores in a project (Research OS, Phase 8M-4, decisions 432 to 437;
//! ADR-011 Parts 6, 7 and 11).** Through the commands' plain bodies, over a root
//! and a workspace session the test makes, as `project_rerun.rs` drives them.
//!
//! * The recipe's kind and role in the store, and what an older build does with
//!   a project that holds them: a kind or role code it does not know, each
//!   pinned on its own (decision 432).
//! * A Save of a run on a prepared slot, and each way it is refused (433).
//! * A rerun rebuilt from the stored recipe, and each way that is refused (434).
//! * The report and the PROV JSON (435), the delete and the caps (437).

mod project_support;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rusqlite::{Connection, OpenFlags};

use app_lib::project_commands::{self as pc, RerunProgress, RerunView, SaveTarget};
use app_lib::project_store::{ArtifactInput, ProjectRoot, RunToSave, SaveRequest};
use app_lib::stats_alpha_commands as alpha;
use app_lib::stats_anova_commands as anova;
use app_lib::stats_corr_commands as corr;
use app_lib::stats_desc_commands as desc;
use app_lib::stats_onesample_commands as onesample;
use app_lib::stats_prep_commands as prep;
use app_lib::stats_ranks_commands as ranks;
use app_lib::stats_ttest_commands as ttest;
use gaply_core::contracts::{CapabilityId, ContentHash, OperationId, RunState};
use gaply_core::research_project::{ArtifactHash, ArtifactKind, DataFormat, ProjectCode, UsedRole};
use gaply_core::stats_alpha::{self, AlphaParams};
use gaply_core::stats_anova::{self, AnovaParams, PostHoc};
use gaply_core::stats_corr::{self, Coefficient, CorrParams, MissingHandling};
use gaply_core::stats_desc::{self, Alias, ColumnRequest, DescribeParams, TreatAs};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_onesample::{self, OneSampleParams, PairedParams};
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};
use gaply_core::stats_ranks::{self, IndependentRanksParams};
use gaply_core::stats_ttest::{self, TTestParams, Variant};

use project_support::*;

/// Four items on 1 to 5, a two-level group, a three-level group and a free
/// column: every method has what it needs among them.
const TABLE: &[u8] = b"q1,q2,q3,q4,arm,site,age\n\
4,2,5,1,a,x,31\n5,1,4,2,a,x,44\n2,4,2,4,a,y,29\n3,3,3,3,a,y,52\n\
1,5,2,5,a,z,37\n4,2,4,2,a,z,41\n5,2,5,1,b,x,63\n2,3,3,3,b,x,35\n\
2,4,1,4,b,y,48\n3,3,4,2,b,y,27\n1,4,2,5,b,z,58\n5,1,5,2,b,z,33\n";

/// A score constant within each of two groups: a t-test on it runs and fails.
const FAILS: &[u8] = b"q1,q2,arm\n1,1,a\n1,1,a\n2,2,b\n2,2,b\n";

const NAME: &str = "a score of mine";

fn alias(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

/// One score of `statistic` over the first `items` columns, of `columns`.
fn a_recipe(columns: u32, name: &str, statistic: Statistic, items: u32) -> Recipe {
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: columns,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: alias(columns + 1),
            name: name.to_string(),
            items: (1..=items).map(alias).collect(),
            reversed: Vec::new(),
            scale: None,
            statistic: Some(statistic),
            rule: Some(MissingRule::EveryItem),
            missing_codes: Vec::new(),
        })],
    }
}

/// The mean of the first three items of [`TABLE`], its score column 8.
fn mean_recipe() -> Recipe {
    a_recipe(7, NAME, Statistic::Mean, 3)
}

fn make(w: &World, slot: u64, recipe: &Recipe) {
    let e = w.engine();
    let v = prep::make(&e.runs, &e.datasets, slot, recipe, 1_000_000)
        .map_err(|_| "the make")
        .unwrap();
    assert!(v.dataset.is_some(), "{:?}", v.errors);
}

/// The eight methods, each with a score in it, as `prep_support::eight`.
const EIGHT: [&str; 8] = [
    "summary",
    "t-test",
    "one-sample",
    "paired",
    "ANOVA",
    "correlations",
    "alpha",
    "rank tests",
];

/// Runs method `which` on the score, column 8 of [`TABLE`], to its end, its
/// spec as a screen builds one on this slot now.
fn run_method(w: &World, slot: u64, which: &str) -> u64 {
    let e = w.engine();
    let held = e.datasets.get(slot).expect("the slot");
    let d = held.dataset();
    let op = OperationId(1);
    let score = alias(8);
    let no_codes = Vec::<f64>::new;
    macro_rules! on_prepared {
        ($spec:expr) => {{
            let mut s = $spec;
            s.preparation = held.preparation();
            s
        }};
    }
    let id = match which {
        "summary" => {
            let params = DescribeParams {
                columns: vec![ColumnRequest {
                    alias: score,
                    treat_as: TreatAs::Numeric,
                }],
                missing_codes: no_codes(),
            };
            desc::run(
                &e.runs,
                &e.datasets,
                slot,
                on_prepared!(stats_desc::spec(op, d, params)),
            )
        }
        "t-test" => {
            let params = TTestParams {
                outcome: score,
                group: alias(5),
                levels: None,
                variant: Some(Variant::Welch),
                missing_codes: no_codes(),
            };
            ttest::run(
                &e.runs,
                &e.datasets,
                slot,
                on_prepared!(stats_ttest::spec(op, d, params)),
            )
        }
        "one-sample" => {
            let params = OneSampleParams {
                outcome: score,
                test_value: Some(3.0),
                missing_codes: no_codes(),
            };
            let spec = on_prepared!(stats_onesample::one_sample_spec(op, d, params));
            onesample::run_one_sample(&e.runs, &e.datasets, slot, spec)
        }
        "paired" => {
            let params = PairedParams {
                first: alias(4),
                second: score,
                missing_codes: no_codes(),
            };
            let spec = on_prepared!(stats_onesample::paired_spec(op, d, params));
            onesample::run_paired(&e.runs, &e.datasets, slot, spec)
        }
        "ANOVA" => {
            let params = AnovaParams {
                outcome: score,
                group: alias(6),
                levels: None,
                post_hoc: vec![PostHoc::Tukey],
                missing_codes: no_codes(),
            };
            anova::run(
                &e.runs,
                &e.datasets,
                slot,
                on_prepared!(stats_anova::spec(op, d, params)),
            )
        }
        "correlations" => {
            let params = CorrParams {
                variables: vec![alias(7), score],
                coefficients: vec![Coefficient::Pearson],
                missing: MissingHandling::Pairwise,
                missing_codes: no_codes(),
            };
            corr::run(
                &e.runs,
                &e.datasets,
                slot,
                on_prepared!(stats_corr::spec(op, d, params)),
            )
        }
        "alpha" => {
            let params = AlphaParams {
                items: vec![alias(4), alias(7), score],
                reversed: Vec::new(),
                scale: None,
                missing_codes: no_codes(),
            };
            alpha::run(
                &e.runs,
                &e.datasets,
                slot,
                on_prepared!(stats_alpha::spec(op, d, params)),
            )
        }
        // Research OS, Phase 8Y-2 (decision 621): the score as the outcome, the
        // file's two-level arm as the group.
        "rank tests" => {
            let params = IndependentRanksParams {
                outcome: score,
                group: alias(5),
                levels: None,
                missing_codes: no_codes(),
            };
            ranks::run(
                &e.runs,
                &e.datasets,
                slot,
                on_prepared!(stats_ranks::spec(op, d, params)),
            )
        }
        other => panic!("no method {other}"),
    }
    .map(|v| v.run_id)
    .unwrap_or_else(|_| panic!("{which}: the run starts"));
    let state = if ["summary", "t-test"].contains(&which) {
        w.wait(id)
    } else {
        w.wait_new_kind(id)
    };
    assert_eq!(state, RunState::Succeeded, "{which}");
    id
}

/// A world with a prepared slot and one saved summary of its score, the project
/// open: (project id, the slot).
fn with_a_prepared_summary(w: &World) -> (String, u64) {
    let slot = w.open("items.csv", TABLE);
    make(w, slot, &mean_recipe());
    let run = run_method(w, slot, "summary");
    let saved = pc::save_run(&w.root(), &w.ws, run, new_project("Prepared")).expect("it saves");
    (saved.project_id, slot)
}

/// The path of the project's one recipe artifact.
fn recipe_file(w: &World, id: &str) -> PathBuf {
    let hash = column(w, id, "SELECT hash FROM artifacts WHERE kind = 'recipe'");
    assert_eq!(hash.len(), 1, "{hash:?}");
    project_dir(w, id).join("artifacts").join(&hash[0])
}

/// Changes one byte of an artifact on disk (its file is 0400, its folder the
/// store's).
#[cfg(unix)]
fn flip_a_byte(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut bytes = std::fs::read(path).unwrap();
    bytes[0] ^= 1;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(path, &bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o400)).unwrap();
}

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

fn project_dir(w: &World, id: &str) -> PathBuf {
    w.root().join("projects").join(id)
}

/// A world whose project, open, holds one saved summary of `CSV`.
fn with_a_summary(w: &World) -> String {
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    pc::save_run(&w.root(), &w.ws, run, new_project("Recipes"))
        .expect("saved")
        .project_id
}

/// The project on disk: every file with its size and SHA-256 (SQLite's `-wal`
/// and `-shm` left out), then each table's row count, then the rows marked
/// damaged.
fn state_of(w: &World, id: &str) -> (BTreeMap<String, (u64, String)>, Vec<i64>) {
    let dir = project_dir(w, id);
    let mut f = files(&dir);
    f.retain(|k, _| !k.ends_with("-wal") && !k.ends_with("-shm"));
    let c = Connection::open_with_flags(dir.join("project.db"), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("project.db");
    let mut counts: Vec<i64> = [
        "project",
        "artifacts",
        "runs",
        "used",
        "generated",
        "derived",
    ]
    .iter()
    .map(|t| {
        c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
            .unwrap()
    })
    .collect();
    counts.push(
        c.query_row(
            "SELECT COUNT(*) FROM artifacts WHERE damage IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap(),
    );
    (f, counts)
}

/// One column of `project.db`, for each row a query gives.
fn column(w: &World, id: &str, sql: &str) -> Vec<String> {
    let c = Connection::open_with_flags(
        project_dir(w, id).join("project.db"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let mut st = c.prepare(sql).unwrap();
    let rows = st
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    rows
}

/// Writes to `project.db` with the project closed and foreign keys off, as a
/// forged or older store could hold it.
fn forge(w: &World, id: &str, sql: &str) {
    let c = Connection::open(project_dir(w, id).join("project.db")).unwrap();
    c.execute_batch(&format!("PRAGMA foreign_keys = OFF; {sql}"))
        .unwrap();
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

/* ---- an older build (decision 432) ------------------------------------------ */

/// Every read of a project's runs through the commands, each by its name.
fn reads(w: &World, id: &str) -> [(&'static str, String); 5] {
    let root = w.root();
    let code_of = |e: Result<(), gaply_core::GaplyError>| match e {
        Ok(()) => "ok".to_string(),
        Err(e) => code(&e),
    };
    [
        ("runs", code_of(pc::runs(&root, &w.ws, id).map(|_| ()))),
        (
            "run_result",
            code_of(pc::run_result(&root, &w.ws, id, 1).map(|_| ())),
        ),
        ("report", code_of(pc::report(&root, &w.ws, id).map(|_| ()))),
        (
            "provenance",
            code_of(pc::provenance(&root, &w.ws, id).map(|_| ())),
        ),
        ("rerun", code_of(pc::rerun(&root, &w.ws, 1).map(|_| ()))),
    ]
}

/// A kind code this build does not know, in an artifact's row, as an older
/// build sees a recipe: every read of the project's runs is refused, and a
/// Save is not, since a Save reads only the artifacts' hashes.
#[test]
fn an_unknown_kind_refuses_every_read_and_changes_nothing_but_not_a_save() {
    let w = World::new("older-kind");
    let id = with_a_summary(&w);
    pc::close(&w.root(), &w.ws).unwrap();
    forge(
        &w,
        &id,
        "UPDATE artifacts SET kind = 'a_kind_from_a_later_build' WHERE kind = 'run_manifest';",
    );
    let opened = pc::open(&w.root(), &w.ws, &id).expect("it still opens");
    assert_eq!(opened.run_count, 1);
    assert_eq!(
        pc::list(&w.root(), &w.ws)
            .unwrap()
            .iter()
            .filter(|e| e.project_id == id && e.refused.is_none())
            .count(),
        1,
        "and is listed"
    );
    let before = state_of(&w, &id);
    for (read, code) in reads(&w, &id) {
        assert_eq!(code, "project.db_unreadable", "{read}");
    }
    assert_eq!(state_of(&w, &id), before, "nothing was changed");
    assert_eq!(pc::current(&w.ws).unwrap().rerun.map(|r| r.run_id), None);

    let slot = w.open("b.csv", CONSTANT);
    let run = w.describe(slot);
    let saved = pc::save_run(&w.root(), &w.ws, run, into_open(&id)).expect("a Save is not refused");
    assert_eq!(saved.stored_run_id, 2);
}

/// A role code this build does not know, in a used row: every read and a Save
/// are refused, and nothing is changed by either.
#[test]
fn an_unknown_role_refuses_every_read_and_a_save_and_changes_nothing() {
    let w = World::new("older-role");
    let id = with_a_summary(&w);
    pc::close(&w.root(), &w.ws).unwrap();
    forge(
        &w,
        &id,
        "UPDATE used SET role = 'a_role_from_a_later_build' WHERE role = 'spec';",
    );
    let opened = pc::open(&w.root(), &w.ws, &id).expect("it still opens");
    assert_eq!(opened.run_count, 1);
    assert_eq!(
        pc::list(&w.root(), &w.ws)
            .unwrap()
            .iter()
            .filter(|e| e.project_id == id && e.refused.is_none())
            .count(),
        1,
        "and is listed"
    );
    let before = state_of(&w, &id);
    for (read, code) in reads(&w, &id) {
        assert_eq!(code, "project.db_unreadable", "{read}");
    }
    let slot = w.open("b.csv", CONSTANT);
    let run = w.describe(slot);
    let e = pc::save_run(&w.root(), &w.ws, run, into_open(&id)).unwrap_err();
    assert_eq!(code(&e), "project.db_unreadable", "the Save");
    assert_eq!(state_of(&w, &id), before, "nothing was changed");
}

/* ---- the edges (decision 432) ------------------------------------------------ */

/// One artifact row of kind `recipe`, its file absent, for an edge to name.
fn a_recipe_row(hash: &str) -> String {
    format!(
        "INSERT INTO artifacts (hash, kind, format, bytes, schema_version, created_at, damage) \
         VALUES ('{hash}', 'recipe', 'json', 1, 1, 1, NULL);"
    )
}

#[test]
fn a_run_with_two_recipe_edges_is_refused_as_an_unknown_artifact() {
    let w = World::new("two-recipes");
    let id = with_a_summary(&w);
    pc::close(&w.root(), &w.ws).unwrap();
    let (a, b) = ("a".repeat(64), "b".repeat(64));
    forge(
        &w,
        &id,
        &format!(
            "{}{}INSERT INTO used (run_id, hash, role) VALUES (1, '{a}', 'recipe'), (1, '{b}', 'recipe');",
            a_recipe_row(&a),
            a_recipe_row(&b)
        ),
    );
    pc::open(&w.root(), &w.ws, &id).unwrap();
    let before = state_of(&w, &id);
    for (read, code) in reads(&w, &id) {
        // The run list reads a manifest only for a rerun, so it has no need
        // of this run's edges.
        let want = if read == "runs" {
            "ok"
        } else {
            "project.unknown_artifact"
        };
        assert_eq!(code, want, "{read}");
    }
    assert_eq!(state_of(&w, &id), before, "nothing was changed");
}

/* ---- a Save (decision 433) ---------------------------------------------------- */

/// Every method's run on a score saves, with its recipe, whatever its result.
#[test]
fn a_run_of_each_method_on_a_score_saves_with_its_recipe() {
    for which in EIGHT {
        let w = World::new("save-eight");
        let slot = w.open("items.csv", TABLE);
        make(&w, slot, &mean_recipe());
        let run = run_method(&w, slot, which);
        let saved = pc::save_run(&w.root(), &w.ws, run, new_project("Eight")).expect(which);
        let id = saved.project_id;
        assert_eq!(
            column(&w, &id, "SELECT hash FROM used WHERE role = 'recipe'"),
            [stats_prep::recipe_hash(&mean_recipe())],
            "{which}"
        );
        let on_disk = std::fs::read(recipe_file(&w, &id)).unwrap();
        assert_eq!(on_disk, stats_prep::recipe_bytes(&mean_recipe()), "{which}");
    }
}

#[test]
fn a_failed_run_on_a_prepared_slot_saves_its_recipe_beside_its_failure() {
    let w = World::new("save-failed");
    let slot = w.open("fails.csv", FAILS);
    let recipe = a_recipe(3, NAME, Statistic::Mean, 2);
    make(&w, slot, &recipe);
    let e = w.engine();
    let held = e.datasets.get(slot).unwrap();
    let mut spec = stats_ttest::spec(
        OperationId(1),
        held.dataset(),
        TTestParams {
            outcome: alias(4),
            group: alias(3),
            levels: None,
            variant: Some(Variant::Welch),
            missing_codes: Vec::new(),
        },
    );
    spec.preparation = held.preparation();
    let run = ttest::run(&e.runs, &e.datasets, slot, spec)
        .map(|v| v.run_id)
        .unwrap_or_else(|_| panic!("it starts"));
    assert_eq!(w.wait(run), RunState::Failed);
    let id = pc::save_run(&w.root(), &w.ws, run, new_project("Failed"))
        .expect("a failed run saves")
        .project_id;
    assert_eq!(
        column(
            &w,
            &id,
            "SELECT role FROM used WHERE run_id = 1 ORDER BY role"
        ),
        ["data", "recipe", "spec"]
    );
    assert_eq!(
        column(
            &w,
            &id,
            "SELECT a.kind FROM generated g JOIN artifacts a ON a.hash = g.hash"
        ),
        ["run_failure"]
    );
    let back = pc::run_result(&w.root(), &w.ws, &id, 1).expect("it reads back");
    assert_eq!(back.state, RunState::Failed);
    assert!(!serde_json::to_string(&back).unwrap().contains(NAME));
}

/// A run keeps its own recipe (Phase 8T-1a, decision 491; ADR-012 question 9):
/// a Save after the slot's recipe was replaced writes the run's recipe, never
/// the slot's. Until 8T-1a this was refused as `project.data_mismatch`.
#[test]
fn a_save_after_the_recipe_was_replaced_writes_the_runs_own_recipe() {
    let w = World::new("save-replaced");
    let slot = w.open("items.csv", TABLE);
    make(&w, slot, &mean_recipe());
    let run = run_method(&w, slot, "summary");
    let replaced = a_recipe(7, NAME, Statistic::Sum, 3);
    make(&w, slot, &replaced);
    let id = pc::save_run(&w.root(), &w.ws, run, new_project("Replaced"))
        .expect("it saves with its own recipe")
        .project_id;
    let stored = std::fs::read(recipe_file(&w, &id)).unwrap();
    assert_eq!(stored, stats_prep::recipe_bytes(&mean_recipe()));
    assert_ne!(stored, stats_prep::recipe_bytes(&replaced));
    assert_eq!(
        column(&w, &id, "SELECT hash FROM artifacts WHERE kind = 'recipe'"),
        [stats_prep::recipe_hash(&mean_recipe())]
    );
}

#[cfg(unix)]
#[test]
fn reading_a_saved_run_verifies_its_recipes_hash_and_returns_nothing_of_it() {
    let w = World::new("read-recipe");
    let (id, _) = with_a_prepared_summary(&w);
    let back = pc::run_result(&w.root(), &w.ws, &id, 1).expect("whole, it reads");
    assert!(!serde_json::to_string(&back).unwrap().contains(NAME));
    flip_a_byte(&recipe_file(&w, &id));
    let e = pc::run_result(&w.root(), &w.ws, &id, 1).unwrap_err();
    assert_eq!(code(&e), "project.artifact_wrong_hash");
    assert_eq!(
        column(
            &w,
            &id,
            "SELECT damage FROM artifacts WHERE kind = 'recipe'"
        ),
        ["project.artifact_wrong_hash"]
    );
}

/// A second run on the same recipe reuses its artifact, by its hash.
#[test]
fn a_second_run_on_the_same_recipe_reuses_its_artifact() {
    let w = World::new("save-twice");
    let (id, slot) = with_a_prepared_summary(&w);
    let run = run_method(&w, slot, "correlations");
    pc::save_run(&w.root(), &w.ws, run, into_open(&id)).expect("it saves");
    assert_eq!(
        column(&w, &id, "SELECT hash FROM artifacts WHERE kind = 'recipe'").len(),
        1
    );
    assert_eq!(
        column(&w, &id, "SELECT hash FROM used WHERE role = 'recipe'").len(),
        2
    );
}

/// The store's own view of the recipe's bytes: `project.hash_mismatch` stays
/// its last check, so bytes that do not hash to the preparation's recipeHash
/// are never kept (decision 433).
#[test]
fn the_store_refuses_a_recipe_whose_bytes_do_not_hash_to_what_is_expected() {
    let dir = TempDir::new("recipe-bytes");
    let store = ProjectRoot::open_root_with(&dir.root(), SMALL).unwrap();
    let mut h = store.create("Recipes").unwrap();
    let (data, spec, bundle) = (
        b"a,b\n1,2\n".to_vec(),
        b"{}".to_vec(),
        b"{\"b\":1}".to_vec(),
    );
    let recipe = stats_prep::recipe_bytes(&mean_recipe());
    let expected = ArtifactHash::parse(&stats_prep::recipe_hash(&mean_recipe())).unwrap();
    let art = |kind, format, bytes: &[u8]| ArtifactInput {
        kind,
        format,
        schema_version: 1,
        expected: ArtifactHash::parse(ContentHash::of_bytes(bytes).as_hex()).unwrap(),
        bytes: bytes.to_vec(),
    };
    let mut lying = art(ArtifactKind::Recipe, DataFormat::Json, &recipe);
    lying.bytes = b"{\"other\":1}".to_vec();
    let d = art(ArtifactKind::DataFile, DataFormat::Csv, &data);
    let s = art(ArtifactKind::OperationSpec, DataFormat::Json, &spec);
    let b = art(ArtifactKind::ResultBundle, DataFormat::Json, &bundle);
    let req = SaveRequest {
        run: RunToSave {
            capability_id: CapabilityId::parse("stats.describe").unwrap(),
            capability_version: 1,
            state: RunState::Succeeded,
            started_at: 1,
            ended_at: 2,
            used: vec![
                (d.expected.clone(), UsedRole::Data),
                (s.expected.clone(), UsedRole::Spec),
                (expected, UsedRole::Recipe),
            ],
            generated: vec![b.expected.clone()],
            derived: Vec::new(),
        },
        artifacts: vec![d, s, b, lying],
    };
    assert_eq!(h.save(req).unwrap_err().code, ProjectCode::HashMismatch);
    assert!(h.runs().unwrap().is_empty(), "nothing was saved");
}

/// And a recipe is JSON: the store's `admits` refuses any other format.
#[test]
fn the_store_refuses_a_recipe_in_any_other_format() {
    let dir = TempDir::new("recipe-format");
    let store = ProjectRoot::open_root_with(&dir.root(), SMALL).unwrap();
    let mut h = store.create("Recipes").unwrap();
    let bytes = stats_prep::recipe_bytes(&mean_recipe());
    let hash = ArtifactHash::parse(&stats_prep::recipe_hash(&mean_recipe())).unwrap();
    let req = SaveRequest {
        run: RunToSave {
            capability_id: CapabilityId::parse("stats.describe").unwrap(),
            capability_version: 1,
            state: RunState::Succeeded,
            started_at: 1,
            ended_at: 2,
            used: vec![(hash.clone(), UsedRole::Recipe)],
            generated: Vec::new(),
            derived: Vec::new(),
        },
        artifacts: vec![ArtifactInput {
            kind: ArtifactKind::Recipe,
            format: DataFormat::Csv,
            schema_version: 1,
            expected: hash,
            bytes,
        }],
    };
    assert_eq!(h.save(req).unwrap_err().code, ProjectCode::BadFormat);
}

/* ---- a rerun (decision 434) --------------------------------------------------- */

/// What a rerun wrote: the files new in the project's artifacts folder.
fn new_artifacts(
    before: &BTreeMap<String, (u64, String)>,
    after: &BTreeMap<String, (u64, String)>,
) -> Vec<String> {
    after
        .keys()
        .filter(|k| k.starts_with("artifacts/") && !before.contains_key(*k))
        .cloned()
        .collect()
}

/// For each of the eight methods: a run on a score, saved, reopened and run
/// again, is identical, with the recipe reused and nothing else written but the
/// new bundle and manifest.
#[test]
fn each_method_on_a_score_saved_reopened_and_rerun_is_identical_and_reuses_the_recipe() {
    use gaply_core::research_project::RerunComparison::Identical;
    for which in EIGHT {
        let w = World::new("rerun-eight");
        let slot = w.open("items.csv", TABLE);
        make(&w, slot, &mean_recipe());
        let run = run_method(&w, slot, which);
        let id = pc::save_run(&w.root(), &w.ws, run, new_project("Eight"))
            .expect(which)
            .project_id;
        pc::close(&w.root(), &w.ws).unwrap();
        pc::open(&w.root(), &w.ws, &id).expect("it reopens");
        let (files_before, _) = state_of(&w, &id);

        pc::rerun(&w.root(), &w.ws, 1).unwrap_or_else(|e| panic!("{which}: {}", code(&e)));
        let r = settled(&w);
        assert_eq!(r.status, RerunProgress::Saved, "{which}");
        assert_eq!(r.stored_run_id, Some(2), "{which}");

        let view = pc::runs(&w.root(), &w.ws, &id).unwrap();
        let again = view.iter().find(|r| r.stored_run_id == 2).unwrap();
        assert_eq!(again.rerun_of, Some(1), "{which}");
        assert_eq!(again.comparison, Some(Identical), "{which}");
        assert_eq!(again.engine_version_differs, Some(false), "{which}");

        // The recipe, by its hash, used by both runs: one artifact.
        let hash = stats_prep::recipe_hash(&mean_recipe());
        assert_eq!(
            column(
                &w,
                &id,
                "SELECT hash FROM used WHERE role = 'recipe' ORDER BY run_id"
            ),
            [hash.clone(), hash.clone()],
            "{which}"
        );
        assert_eq!(
            column(&w, &id, "SELECT hash FROM artifacts WHERE kind = 'recipe'"),
            [hash],
            "{which}"
        );
        // Nothing written but the new bundle and manifest. Either may already
        // be there, by its hash: a manifest with the same elapsed time has the
        // same bytes, and is reused as any artifact is.
        let (files_after, _) = state_of(&w, &id);
        let new = new_artifacts(&files_before, &files_after);
        assert!(!new.is_empty() && new.len() <= 2, "{which}: {new:?}");
        let mut made = column(
            &w,
            &id,
            "SELECT a.kind FROM generated g JOIN artifacts a ON a.hash = g.hash WHERE g.run_id = 2 ORDER BY a.kind",
        );
        made.dedup();
        assert_eq!(made, ["result_bundle", "run_manifest"], "{which}");
        for name in new {
            let hash = name.strip_prefix("artifacts/").unwrap();
            assert!(
                column(
                    &w,
                    &id,
                    &format!(
                        "SELECT g.hash FROM generated g WHERE g.run_id = 2 AND g.hash = '{hash}'"
                    )
                )
                .len()
                    == 1,
                "{which}: {name}"
            );
        }
    }
}

/// A rerun's one-run slot gives the same Preparation as the live slot did, and
/// holds the one prepared table (decision 434).
#[test]
fn the_one_run_slot_of_a_prepared_rerun_gives_the_live_slots_preparation() {
    use app_lib::runs::{Slot, SourceFile};
    use gaply_core::stats_adapters;
    let w = World::new("one-run-slot");
    let slot = w.open("items.csv", TABLE);
    make(&w, slot, &mean_recipe());
    let live = w.engine().datasets.get(slot).unwrap();
    let live_preparation = live.preparation().expect("a live preparation");

    let file = app_lib::stats_l1_ingest::load_dataset_from_bytes(TABLE, DataFormat::Csv).unwrap();
    let original = file.schema.fingerprint.clone();
    let prepared = stats_adapters::prep_build(
        &mean_recipe(),
        app_lib::workspace::build(),
        &file,
        1_000_000,
    )
    .unwrap();
    let source = SourceFile::new(
        TABLE.to_vec(),
        ContentHash::of_bytes(TABLE),
        DataFormat::Csv,
    );
    let one = Slot::for_one_run_prepared(prepared, original, source);
    assert_eq!(one.preparation(), Some(live_preparation));
    assert!(one.prepared().is_some(), "the table is the prepared one");
    assert_eq!(
        stats_desc::input_ref(one.dataset()),
        stats_desc::input_ref(live.dataset())
    );
    assert_eq!(one.source().unwrap().bytes(), TABLE);
}

/// The artifact row and file of the project's recipe replaced by `bytes`, with
/// the row's and the edge's hash changed to match: only what comes after the
/// hash check can catch it. The project is closed first and open after.
fn swap_the_recipe(w: &World, id: &str, bytes: &[u8]) {
    let old = recipe_file(w, id);
    let old_hash = old.file_name().unwrap().to_string_lossy().into_owned();
    let new_hash = ContentHash::of_bytes(bytes).as_hex().to_string();
    pc::close(&w.root(), &w.ws).unwrap();
    std::fs::write(old.with_file_name(&new_hash), bytes).unwrap();
    std::fs::remove_file(&old).unwrap();
    forge(
        w,
        id,
        &format!(
            "UPDATE artifacts SET hash = '{new_hash}', bytes = {} WHERE hash = '{old_hash}';
             UPDATE used SET hash = '{new_hash}' WHERE hash = '{old_hash}';",
            bytes.len()
        ),
    );
    pc::open(&w.root(), &w.ws, id).unwrap();
}

/// A rerun that must be refused with `want`, changing nothing.
fn refused_with_nothing_changed(w: &World, id: &str, want: &str) {
    let before = state_of(w, id);
    let e = pc::rerun(&w.root(), &w.ws, 1).unwrap_err();
    assert_eq!(code(&e), want);
    assert_eq!(state_of(w, id), before, "{want}: nothing was changed");
    assert_eq!(w.ws.projects().rerun_going(), None, "{want}: nothing going");
}

#[cfg(unix)]
#[test]
fn a_recipe_with_one_byte_changed_is_refused_with_only_its_damage_mark() {
    let w = World::new("rerun-byte");
    let (id, _) = with_a_prepared_summary(&w);
    let file = recipe_file(&w, &id);
    flip_a_byte(&file);
    let (files_before, mut counts_before) = state_of(&w, &id);
    let e = pc::rerun(&w.root(), &w.ws, 1).unwrap_err();
    assert_eq!(code(&e), "project.artifact_wrong_hash");
    let (files_after, counts_after) = state_of(&w, &id);
    assert_eq!(files_after, files_before, "no file was written");
    *counts_before.last_mut().unwrap() += 1;
    assert_eq!(
        counts_after, counts_before,
        "every row as it was but one damage mark"
    );
    assert_eq!(
        column(
            &w,
            &id,
            "SELECT damage FROM artifacts WHERE kind = 'recipe'"
        ),
        ["project.artifact_wrong_hash"]
    );
    assert_eq!(w.ws.projects().rerun_going(), None);
}

#[test]
fn another_valid_recipe_in_its_place_is_refused_by_the_prepared_input_hash() {
    let w = World::new("rerun-other");
    let (id, _) = with_a_prepared_summary(&w);
    // Of the same width and valid, so it builds: only the input hash differs.
    let other = stats_prep::recipe_bytes(&a_recipe(7, NAME, Statistic::Sum, 3));
    swap_the_recipe(&w, &id, &other);
    refused_with_nothing_changed(&w, &id, "project.rerun_refused");
}

#[test]
fn a_recipe_edge_to_a_hash_with_no_row_is_refused_as_an_unknown_artifact() {
    let w = World::new("rerun-no-row");
    let (id, _) = with_a_prepared_summary(&w);
    pc::close(&w.root(), &w.ws).unwrap();
    forge(
        &w,
        &id,
        &format!(
            "UPDATE used SET hash = '{}' WHERE role = 'recipe';",
            "f".repeat(64)
        ),
    );
    pc::open(&w.root(), &w.ws, &id).unwrap();
    refused_with_nothing_changed(&w, &id, "project.unknown_artifact");
}

/// A spec with a preparation and no recipe edge reads back and validates against
/// the file's table, which is not its input.
#[test]
fn a_prepared_run_with_no_recipe_edge_is_refused_and_changes_nothing() {
    let w = World::new("rerun-no-edge");
    let (id, _) = with_a_prepared_summary(&w);
    pc::close(&w.root(), &w.ws).unwrap();
    forge(&w, &id, "DELETE FROM used WHERE role = 'recipe';");
    pc::open(&w.root(), &w.ws, &id).unwrap();
    refused_with_nothing_changed(&w, &id, "project.rerun_refused");
}

/// Stored bytes that are no canonical recipe, and a recipe a validation refuses,
/// are the one closed code.
#[test]
fn a_recipe_that_will_not_parse_or_validate_is_unusable_and_changes_nothing() {
    let w = World::new("rerun-unusable");
    let (id, _) = with_a_prepared_summary(&w);
    let canonical = stats_prep::recipe_bytes(&mean_recipe());
    let mut spaced = b" ".to_vec();
    spaced.extend_from_slice(&canonical);
    swap_the_recipe(&w, &id, &spaced);
    refused_with_nothing_changed(&w, &id, "project.recipe_unusable");

    // Written for a file of another width.
    let wrong = stats_prep::recipe_bytes(&a_recipe(6, NAME, Statistic::Mean, 3));
    swap_the_recipe(&w, &id, &wrong);
    refused_with_nothing_changed(&w, &id, "project.recipe_unusable");

    // The statistic left open.
    let mut open = mean_recipe();
    let RecipeStep::ScaleScore(step) = &mut open.steps[0];
    step.statistic = None;
    swap_the_recipe(&w, &id, &stats_prep::recipe_bytes(&open));
    refused_with_nothing_changed(&w, &id, "project.recipe_unusable");

    // No row can have the score: every item's every value a missing code.
    let mut none = mean_recipe();
    let RecipeStep::ScaleScore(step) = &mut none.steps[0];
    step.missing_codes = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    swap_the_recipe(&w, &id, &stats_prep::recipe_bytes(&none));
    refused_with_nothing_changed(&w, &id, "project.recipe_unusable");
}

/* ---- the report and the PROV JSON (decision 435) ------------------------------ */

/// A mean of three items, one reversed, at least two needed, one code.
fn a_full_recipe() -> Recipe {
    let mut r = mean_recipe();
    let RecipeStep::ScaleScore(step) = &mut r.steps[0];
    step.reversed = vec![alias(2)];
    step.scale = Some(stats_alpha::ScaleEnds { min: 1.0, max: 5.0 });
    step.rule = Some(MissingRule::AtLeast(2));
    step.missing_codes = vec![9.0];
    r
}

fn a_project_of(w: &World, recipe: &Recipe) -> String {
    let slot = w.open("items.csv", TABLE);
    make(w, slot, recipe);
    let run = run_method(w, slot, "summary");
    pc::save_run(&w.root(), &w.ws, run, new_project("Report"))
        .expect("it saves")
        .project_id
}

#[test]
fn the_report_says_how_a_score_was_made_by_position_and_never_by_name() {
    let w = World::new("report-lines");
    let recipe = a_full_recipe();
    let id = a_project_of(&w, &recipe);
    let report = String::from_utf8(pc::report(&w.root(), &w.ws, &id).unwrap().bytes).unwrap();
    for want in [
        "- Data file as read: 12 rows, 7 columns; 1 prepared column\n",
        &format!(
            "- Prepared by {} version 1, engine version 1\n",
            stats_prep::CAPABILITY_ID
        ),
        "- Prepared column 8: mean of columns 1, 2 and 3; column 2 reversed on 1 to 5; at least 2 items; 1 missing code\n",
        "- Column 8: summarised_numeric\n",
        "| used, recipe | recipe | json |",
    ] {
        assert!(report.contains(want), "missing `{want}` in:\n{report}");
    }
    assert!(!report.contains("8 columns"), "{report}");
    assert!(!report.contains(NAME), "{report}");
    let hash = stats_prep::recipe_hash(&recipe);
    assert!(report.contains(&format!("`{hash}`")), "its fingerprint");
}

#[test]
fn the_report_of_every_other_rule_and_a_project_with_two_scores() {
    let w = World::new("report-two");
    let mut recipe = mean_recipe();
    let RecipeStep::ScaleScore(first) = &mut recipe.steps[0];
    first.statistic = Some(Statistic::Sum);
    let mut second = match &recipe.steps[0] {
        RecipeStep::ScaleScore(s) => s.clone(),
    };
    second.alias = alias(9);
    second.name = "another of mine".into();
    second.statistic = Some(Statistic::Mean);
    second.items = vec![alias(1), alias(2)];
    second.reversed = vec![alias(1), alias(2)];
    second.scale = Some(stats_alpha::ScaleEnds { min: 0.5, max: 5.5 });
    second.rule = Some(MissingRule::AtLeast(1));
    second.missing_codes = vec![8.0, 9.0];
    recipe.steps.push(RecipeStep::ScaleScore(second));
    let id = a_project_of(&w, &recipe);
    let report = String::from_utf8(pc::report(&w.root(), &w.ws, &id).unwrap().bytes).unwrap();
    for want in [
        "- Data file as read: 12 rows, 7 columns; 2 prepared columns\n",
        "- Prepared column 8: sum of columns 1, 2 and 3; every item; no missing codes\n",
        "- Prepared column 9: mean of columns 1 and 2; columns 1 and 2 reversed on 0.5 to 5.5; at least 1 item; 2 missing codes\n",
    ] {
        assert!(report.contains(want), "missing `{want}` in:\n{report}");
    }
}

#[test]
fn the_prov_json_has_the_recipes_entity_its_used_edge_and_its_derivation() {
    let w = World::new("prov-recipe");
    let recipe = mean_recipe();
    let id = a_project_of(&w, &recipe);
    let prov = pc::provenance(&w.root(), &w.ws, &id).unwrap();
    let text = String::from_utf8(prov.bytes).unwrap();
    assert!(!text.contains(NAME), "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let hash = stats_prep::recipe_hash(&recipe);
    let entity = format!("gaply:artifact-{hash}");
    assert_eq!(v["entity"][&entity]["prov:type"], "gaply:recipe");
    assert_eq!(v["entity"][&entity]["gaply:format"], "json");
    let used = &v["used"][format!("_:used-1-{hash}-recipe")];
    assert_eq!(used["prov:role"], "gaply:recipe");
    assert_eq!(used["prov:entity"], entity.as_str());
    assert_eq!(used["prov:activity"], "gaply:run-1");
    let from_recipe: Vec<&serde_json::Value> = v["wasDerivedFrom"]
        .as_object()
        .unwrap()
        .values()
        .filter(|d| d["prov:usedEntity"] == entity.as_str())
        .collect();
    assert_eq!(from_recipe.len(), 2, "the bundle and the manifest");
    assert_eq!(
        v["wasDerivedFrom"].as_object().unwrap().len(),
        2 * 3,
        "each from the data file, the spec and the recipe"
    );
}

/// A row saved beside the run: its spec's preparation names `recipe`'s hash and
/// its used edges are as `edges` says, as a forged or damaged project could be.
fn store_a_prepared_row(w: &World, id: &str, recipe: Option<&[u8]>) -> u64 {
    let spec = pc::run_result(&w.root(), &w.ws, id, 1).unwrap().spec_json;
    let mut spec: serde_json::Value = serde_json::from_str(&spec).unwrap();
    let recipe_hash = recipe.map_or("e".repeat(64), |r| {
        ContentHash::of_bytes(r).as_hex().to_string()
    });
    spec["preparation"]["recipeHash"] = recipe_hash.clone().into();
    let spec = serde_json::to_vec(&spec).unwrap();
    let art = |kind, format, bytes: &[u8]| ArtifactInput {
        kind,
        format,
        schema_version: 1,
        expected: ArtifactHash::parse(ContentHash::of_bytes(bytes).as_hex()).unwrap(),
        bytes: bytes.to_vec(),
    };
    let d = art(ArtifactKind::DataFile, DataFormat::Csv, TABLE);
    let s = art(ArtifactKind::OperationSpec, DataFormat::Json, &spec);
    let mut used = vec![
        (d.expected.clone(), UsedRole::Data),
        (s.expected.clone(), UsedRole::Spec),
    ];
    let mut artifacts = vec![d, s];
    if let Some(r) = recipe {
        let a = art(ArtifactKind::Recipe, DataFormat::Json, r);
        used.push((a.expected.clone(), UsedRole::Recipe));
        artifacts.push(a);
    }
    pc::close(&w.root(), &w.ws).unwrap();
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let mut h = store.open(id).unwrap();
    let saved = h
        .save(SaveRequest {
            run: RunToSave {
                capability_id: CapabilityId::parse("stats.describe").unwrap(),
                capability_version: 1,
                state: RunState::Failed,
                started_at: 1,
                ended_at: 2,
                used,
                generated: Vec::new(),
                derived: Vec::new(),
            },
            artifacts,
        })
        .unwrap();
    h.close();
    pc::open(&w.root(), &w.ws, id).unwrap();
    saved.0
}

fn exports_refused(w: &World, id: &str, want: &str) {
    let before = state_of(w, id);
    let e = pc::report(&w.root(), &w.ws, id).map(|_| ()).unwrap_err();
    assert_eq!(code(&e), want, "the report");
    let e = pc::provenance(&w.root(), &w.ws, id)
        .map(|_| ())
        .unwrap_err();
    assert_eq!(code(&e), want, "the provenance");
    assert_eq!(state_of(w, id), before, "nothing was changed");
}

#[test]
fn an_export_refuses_a_recipe_that_will_not_parse_as_it_does_a_spec() {
    let w = World::new("export-unparsed");
    let id = a_project_of(&w, &mean_recipe());
    store_a_prepared_row(&w, &id, Some(b"{\"not\":\"a recipe\"}"));
    exports_refused(&w, &id, "project.db_unreadable");
}

#[test]
fn an_export_refuses_a_preparation_with_no_recipe_or_a_recipe_with_no_preparation() {
    let w = World::new("export-no-recipe");
    let id = a_project_of(&w, &mean_recipe());
    store_a_prepared_row(&w, &id, None);
    exports_refused(&w, &id, "project.db_unreadable");

    let w = World::new("export-no-preparation");
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    let id = pc::save_run(&w.root(), &w.ws, run, new_project("Plain"))
        .unwrap()
        .project_id;
    let bytes = stats_prep::recipe_bytes(&mean_recipe());
    let hash = stats_prep::recipe_hash(&mean_recipe());
    pc::close(&w.root(), &w.ws).unwrap();
    std::fs::write(project_dir(&w, &id).join("artifacts").join(&hash), &bytes).unwrap();
    forge(
        &w,
        &id,
        &format!(
            "{}INSERT INTO used (run_id, hash, role) VALUES (1, '{hash}', 'recipe');",
            a_recipe_row(&hash)
                .replace(", 1, 1, 1, NULL", &format!(", {}, 1, 1, NULL", bytes.len()))
        ),
    );
    pc::open(&w.root(), &w.ws, &id).unwrap();
    exports_refused(&w, &id, "project.db_unreadable");
}

/// The recipe a report describes is the one the run's spec names: another valid
/// one, swapped in with its row's hash to match, is not described as if it had
/// made the table.
#[test]
fn an_export_refuses_another_recipe_than_the_one_the_spec_names() {
    let w = World::new("export-swapped");
    let id = a_project_of(&w, &mean_recipe());
    swap_the_recipe(
        &w,
        &id,
        &stats_prep::recipe_bytes(&a_recipe(7, NAME, Statistic::Sum, 3)),
    );
    exports_refused(&w, &id, "project.db_unreadable");
}

/* ---- the delete, the caps and a project with no recipe (decision 437) ---------- */

#[test]
fn a_delete_removes_the_recipe_with_the_rest() {
    let w = World::new("delete-recipe");
    let (id, _) = with_a_prepared_summary(&w);
    let file = recipe_file(&w, &id);
    assert!(file.exists());
    let dir = project_dir(&w, &id);
    pc::close(&w.root(), &w.ws).unwrap();
    pc::open(&w.root(), &w.ws, &id).unwrap();
    let c = pc::prepare_delete(&w.root(), &w.ws, &id).unwrap();
    pc::delete(&w.root(), &w.ws, &id, Some(&c.confirmation)).unwrap();
    assert!(!file.exists(), "the recipe's file is gone");
    assert!(!dir.exists(), "and the project's folder");
    assert!(
        pc::list(&w.root(), &w.ws)
            .unwrap()
            .iter()
            .all(|e| e.project_id != id),
        "and it is no longer listed"
    );
}

/// What a Save of `artifacts` would need, read from a one-byte cap's refusal.
fn needed_for(tag: &str, with_recipe: bool) -> u64 {
    let dir = TempDir::new(tag);
    let caps = gaply_core::research_project::Caps {
        project: 1,
        all_projects: 1 << 40,
        db_allowance: 0,
    };
    let store = ProjectRoot::open_root_with(&dir.root(), caps).unwrap();
    let mut h = store.create("Caps").unwrap();
    let art = |kind, format, bytes: &[u8]| ArtifactInput {
        kind,
        format,
        schema_version: 1,
        expected: ArtifactHash::parse(ContentHash::of_bytes(bytes).as_hex()).unwrap(),
        bytes: bytes.to_vec(),
    };
    let d = art(ArtifactKind::DataFile, DataFormat::Csv, TABLE);
    let mut used = vec![(d.expected.clone(), UsedRole::Data)];
    let mut artifacts = vec![d];
    if with_recipe {
        let r = art(
            ArtifactKind::Recipe,
            DataFormat::Json,
            &stats_prep::recipe_bytes(&mean_recipe()),
        );
        used.push((r.expected.clone(), UsedRole::Recipe));
        artifacts.push(r);
    }
    let refused = h
        .save(SaveRequest {
            run: RunToSave {
                capability_id: CapabilityId::parse("stats.describe").unwrap(),
                capability_version: 1,
                state: RunState::Failed,
                started_at: 1,
                ended_at: 2,
                used,
                generated: Vec::new(),
                derived: Vec::new(),
            },
            artifacts,
        })
        .unwrap_err();
    assert_eq!(refused.code, ProjectCode::ProjectCap);
    refused.needed.expect("a cap says what it needed")
}

#[test]
fn the_caps_count_a_recipes_bytes() {
    let without = needed_for("caps-without", false);
    let with = needed_for("caps-with", true);
    assert_eq!(
        with - without,
        stats_prep::recipe_bytes(&mean_recipe()).len() as u64,
        "exactly its bytes"
    );
}

#[test]
fn a_project_saved_with_no_recipe_lists_opens_reads_exports_and_reruns_as_before() {
    use gaply_core::research_project::RerunComparison::Identical;
    let w = World::new("no-recipe");
    let id = with_a_summary(&w);
    assert_eq!(
        column(&w, &id, "SELECT kind FROM artifacts ORDER BY kind"),
        [
            "data_file",
            "operation_spec",
            "result_bundle",
            "run_manifest"
        ]
    );
    assert_eq!(
        column(&w, &id, "SELECT role FROM used ORDER BY role"),
        ["data", "spec"]
    );
    pc::close(&w.root(), &w.ws).unwrap();
    assert_eq!(
        pc::list(&w.root(), &w.ws)
            .unwrap()
            .iter()
            .filter(|e| e.project_id == id && e.refused.is_none())
            .count(),
        1
    );
    assert_eq!(pc::open(&w.root(), &w.ws, &id).unwrap().run_count, 1);
    let back = pc::run_result(&w.root(), &w.ws, &id, 1).unwrap();
    assert!(!back.spec_json.contains("preparation"));
    assert!(!back.bundle_json.unwrap().contains("preparation"));
    let report = String::from_utf8(pc::report(&w.root(), &w.ws, &id).unwrap().bytes).unwrap();
    assert!(
        report.contains("- Data file as read: 6 rows, 2 columns\n"),
        "{report}"
    );
    assert!(!report.contains("repared"), "{report}");
    assert!(!report.contains("recipe"), "{report}");
    let prov = String::from_utf8(pc::provenance(&w.root(), &w.ws, &id).unwrap().bytes).unwrap();
    assert!(!prov.contains("recipe"), "{prov}");

    pc::rerun(&w.root(), &w.ws, 1).unwrap();
    assert_eq!(settled(&w).status, RerunProgress::Saved);
    let view = pc::runs(&w.root(), &w.ws, &id).unwrap();
    assert_eq!(view[1].comparison, Some(Identical));
    assert!(column(&w, &id, "SELECT hash FROM used WHERE role = 'recipe'").is_empty());
}
