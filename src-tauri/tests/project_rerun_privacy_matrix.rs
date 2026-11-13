//! **The privacy matrix with a rerun (Research OS, Phase 5C-1a; decisions 186
//! and 190), in the pattern of `stats_desc_privacy_matrix.rs`.** Its seven
//! sentinels, and a project named by an eighth:
//!
//! * a header, `HDR_SENTINEL_q81`, of the selected numeric column;
//! * a level label, `LVL_SENTINEL_k44`, of the selected categorical column;
//! * the file name `FILE_SENTINEL_d17.csv`, in the folder `PATH_SENTINEL_w93`;
//! * `7123.4567`, in a numeric column the user does not select (the
//!   correlations, since Phase 8H-2, select it as their second variable,
//!   and Cronbach's alpha, since 8I-2, as its second item: their results
//!   hold aliases, codes and numbers, and its column's mean and SD, and the
//!   scale's mean and variance without either item, are neither 7123.4567
//!   nor 61.8129);
//! * `61.8129`, in the selected numeric column (no statistic can be it: the
//!   describe matrix's own data, checked there against R). Since Phase 8G-2
//!   one row (95.5) is in the label's group, so under the one-way ANOVA too,
//!   which reports each group's min and max, it is neither: its group is
//!   10.5, 95.5, 61.8129, 50 and 31;
//! * `TXT_SENTINEL_m02`, a text cell in a column the user does not select;
//! * since Phase 8X-2 (decision 590), `LV2_SENTINEL_r35`, a level label of
//!   crosstabs' second column, on a table of its own (below);
//! * the project's name, `TITLESENTp22`.
//!
//! The flow, through the commands' plain functions: open the file, run a
//! summary, save it into a new project, run it again, let the save at the
//! end land; since Phase 8G-2 (decision 330) the same for a one-way ANOVA of
//! the selected column by the label's column, all three post hoc tests,
//! saved into the open project and run again; since Phase 8H-2 (decision
//! 357) the same for correlations of the selected column and `aux`, all
//! three coefficients, pairwise; since Phase 8I-2 (decision 382) the same for
//! Cronbach's alpha of the selected column and `aux`, nothing reversed; since
//! Phase 8X-2 (decision 590), after the prepared runs below, crosstabs of a
//! second table's two text columns, headed by the header sentinel and holding
//! LVL and LV2 among their levels and the code -99, ticked, the file named
//! `FILE_SENTINEL_d17_xtab.csv` in the same folder, saved as run 14 and run
//! again as 15, so no run id above moves; read the project back (`project_current`, `project_runs`, each
//! run's result) and render both exports, then delete the project.
//!
//! * In the app's data folder, the sentinels of the data appear only inside
//!   `projects/<id>/artifacts/`, and the name only in `project.db`, where
//!   `project.title` is; the file's name and its folder appear in no file.
//!   Every other file of the folder (a migrated `gaply.db`, which the test
//!   makes first) is byte for byte as it was.
//! * No tracing line, from any thread (the rerun's save at the end runs on
//!   its worker), holds any sentinel.
//! * No response holds one, but a run's result read back (the header and the
//!   label, as 5B allows) and the report's title (the name).
//! * After the project is deleted, no file in the folder holds any.
//!
//! **A run on prepared data (Research OS, Phase 8M-4, decision 437), with 8M-3's
//! three sentinels.** A summary and Cronbach's alpha of a score, saved and run
//! again, then a Save after a second make that keeps the run's own recipe
//! (Phase 8T-1a, decision 491), a Save refused after the file's close, and a
//! rerun refused. The typed name is only in the
//! recipe artifact, the `DatasetView`, and, since Phase 8U-3 (decision 553),
//! `project_recipes`' response, read in memory for the project page before
//! the recipe is damaged; `project_runs`' response holds none of the three;
//! an item's value only in the data file's
//! artifact and the `DatasetView`; the score's value only in the `DatasetView`.
//! All three are absent from `project.db`'s bytes, every spec, manifest, bundle,
//! the report, the PROV JSON, every refusal and every log line. The negative
//! control is a score constant at the sentinel, saved, whose mean the summary's
//! result shows.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global
//! default for the whole process.

mod desc_support;
mod l1_support;
mod project_support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use app_lib::project_commands::{self as pc, RerunProgress, SaveTarget};
use app_lib::stats_desc_commands as desc;
use app_lib::stats_prep_commands as prep;
use desc_support::{delimited, C};
use gaply_core::contracts::{CapabilityId, OperationId, RunState};
use gaply_core::stats_alpha::{self, AlphaParams};
use gaply_core::stats_anova::{self, AnovaParams, PostHoc};
use gaply_core::stats_corr::{self, Coefficient, CorrParams, MissingHandling};
use gaply_core::stats_desc::{self, Alias, ColumnRequest, DescribeParams, TreatAs};
use gaply_core::stats_factor::{self, Extraction, FactorParams, Rotation};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_linear_regression::{self, LinearRegressionParams};
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};
use gaply_core::stats_ranks::{self, IndependentRanksParams};
use gaply_core::stats_related_ranks::{self, RelatedRanksParams};
use gaply_core::stats_xtab::{self, CrosstabsParams};
use gaply_core::Database;
use project_support::World;

const HDR: &str = "HDR_SENTINEL_q81";
const LVL: &str = "LVL_SENTINEL_k44";
const FILE: &str = "FILE_SENTINEL_d17";
const PATH: &str = "PATH_SENTINEL_w93";
const UNSEL_NUM: &str = "7123.4567";
const SEL_NUM: &str = "61.8129";
const TXT: &str = "TXT_SENTINEL_m02";
const TITLE: &str = "TITLESENTp22";
// Research OS, Phase 8X-2 (decision 590): a level label of crosstabs' second
// column, on its own table.
const LV2: &str = "LV2_SENTINEL_r35";

const SENTINELS: [&str; 8] = [HDR, LVL, FILE, PATH, UNSEL_NUM, SEL_NUM, TXT, LV2];

// 8M-3's (stats_prep_privacy_matrix.rs), on a binary grid: 61.8125 is an item's
// value in one row, and the score there, the mean of it and 85.25, is 73.53125.
const NAME: &str = "NAME_SENTINEL_f19";
const ITEM_VALUE: &str = "61.8125";
const SCORE_VALUE: &str = "73.53125";

fn table() -> Vec<u8> {
    let headers = [HDR, "group", "note", "aux"];
    let score = [
        "44.1257", "10.5", "95.5", SEL_NUM, "23.25", "50", "82", "31", "70.75",
    ];
    let group = ["a", LVL, LVL, LVL, "a", LVL, "a", LVL, "a"];
    let note = ["n1", "n2", TXT, "n4", "n5", "n6", "n7", "n8", "n9"];
    let aux = ["1", "2", "3", "4", UNSEL_NUM, "6", "7", "8", "9"];
    let rows: Vec<Vec<C>> = (0..9)
        .map(|i| vec![C::N(score[i]), C::T(group[i]), C::T(note[i]), C::N(aux[i])])
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// Crosstabs' table (Research OS, Phase 8X-2, decision 590): two text columns,
/// the first headed by the header sentinel with LVL among its levels and one
/// cell the code -99, the second with LV2 among its levels. Its own table,
/// since `table()`'s only other text column holds TXT, which no result may
/// hold.
fn xtab_table() -> Vec<u8> {
    let headers = [HDR, "side"];
    let first = ["a", LVL, "a", LVL, "-99", "a", LVL, "a", LVL, LVL];
    let second = ["p", LV2, LV2, "p", "p", LV2, "p", LV2, "p", LV2];
    let rows: Vec<Vec<C>> = (0..10)
        .map(|i| {
            let cell = if first[i] == "-99" {
                C::N(first[i])
            } else {
                C::T(first[i])
            };
            vec![cell, C::T(second[i])]
        })
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// The Friedman test's table (Research OS, Phase 8Z-2, decision 655): three numeric
/// columns, the first headed by the header sentinel, and a fourth, not chosen, holding
/// the unselected number. Its own table, since `table()` has two numeric columns.
fn friedman_table() -> Vec<u8> {
    let headers = [HDR, "two", "three", "aux"];
    let first = ["1.5", "9.25", "3", "7.5", "5", "2.75"];
    let second = ["4.5", "2", "8.25", "6", "1", "9"];
    let third = ["7", "6.5", "4", "2.5", "10.25", "3.5"];
    let aux = ["1", "2", UNSEL_NUM, "4", "5", "6"];
    let rows: Vec<Vec<C>> = (0..6)
        .map(|i| vec![C::N(first[i]), C::N(second[i]), C::N(third[i]), C::N(aux[i])])
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// The table of 8M-3's matrix, with plain headers: the name is the one sentinel
/// a person typed.
fn prepared_table() -> Vec<u8> {
    let headers = ["q1", "q2", "q3", "q4", "arm", "site", "age"];
    let q1 = [
        "44.5", "10.5", "23.25", ITEM_VALUE, "50", "95.5", "27.75", "31", "70.75", "12", "66",
        "18.5",
    ];
    let q2 = [
        "3.5", "11.25", "2", "85.25", "16", "99", "8.5", "4", "99", "1.5", "6", "2.25",
    ];
    let q3 = ["1", "2", "3", "4", "5", "1", "2", "3", "4", "5", "1", "2"];
    let q4 = ["2", "1", "4", "3", "5", "2", "2", "5", "1", "4", "3", "1"];
    let arm = ["a", "a", "a", "a", "a", "a", "b", "b", "b", "b", "b", "b"];
    let site = ["x", "x", "y", "y", "z", "z", "x", "x", "y", "y", "z", "z"];
    let age = [
        "31", "44", "29", "52", "37", "41", "63", "35", "48", "27", "58", "33",
    ];
    let rows: Vec<Vec<C>> = (0..12)
        .map(|i| {
            vec![
                C::N(q1[i]),
                C::N(q2[i]),
                C::N(q3[i]),
                C::N(q4[i]),
                C::T(arm[i]),
                C::T(site[i]),
                C::N(age[i]),
            ]
        })
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

fn prepared_recipe(columns: u32, name: &str) -> Recipe {
    let alias = |n: u32| Alias(ColumnAlias(n));
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: columns,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: alias(columns + 1),
            name: name.to_string(),
            items: vec![alias(1), alias(2)],
            reversed: Vec::new(),
            scale: None,
            statistic: Some(Statistic::Mean),
            rule: Some(MissingRule::EveryItem),
            missing_codes: Vec::new(),
        })],
    }
}

/// A summary of column `score` on the prepared `slot`, run to its end.
fn summary_of(w: &World, slot: u64, score: u32) -> u64 {
    let e = w.engine();
    let held = e.datasets.get(slot).unwrap();
    let mut spec = stats_desc::spec(
        OperationId(5),
        held.dataset(),
        DescribeParams {
            columns: vec![ColumnRequest {
                alias: Alias(ColumnAlias(score)),
                treat_as: TreatAs::Numeric,
            }],
            missing_codes: vec![],
        },
    );
    spec.preparation = held.preparation();
    drop(held);
    let run = desc::run(&e.runs, &e.datasets, slot, spec).unwrap().run_id;
    w.wait(run);
    run
}

/// Cronbach's alpha of two items and the score, on the prepared `slot`.
fn alpha_of(w: &World, slot: u64) -> u64 {
    let e = w.engine();
    let held = e.datasets.get(slot).unwrap();
    let alias = |n: u32| Alias(ColumnAlias(n));
    let mut spec = stats_alpha::spec(
        OperationId(6),
        held.dataset(),
        AlphaParams {
            items: vec![alias(4), alias(7), alias(8)],
            reversed: vec![],
            scale: None,
            missing_codes: vec![],
        },
    );
    spec.preparation = held.preparation();
    drop(held);
    let run = app_lib::stats_alpha_commands::run(&e.runs, &e.datasets, slot, spec)
        .unwrap()
        .run_id;
    w.wait_new_kind(run);
    run
}

/// A rerun of stored run `of`, saved as run `of + 1`.
fn rerun_saved(w: &World, app: &Path, of: u64) -> String {
    let started = pc::rerun(app, &w.ws, of).expect("the rerun starts");
    let start = std::time::Instant::now();
    let current = loop {
        let c = pc::current(&w.ws).unwrap();
        if c.rerun
            .as_ref()
            .is_some_and(|r| r.status != RerunProgress::Going && r.stored_run_id == Some(of + 1))
        {
            break c;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(20));
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(current.rerun.as_ref().unwrap().status, RerunProgress::Saved);
    format!(
        "{}{}",
        serde_json::to_string(&started).unwrap(),
        serde_json::to_string(&current).unwrap()
    )
}

/// Each of the prepared run's sentinels `bytes` holds, as text, and each number
/// also as the 8-byte big-endian double SQLite stores.
fn prepared_found(bytes: &[u8]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = [NAME, ITEM_VALUE, SCORE_VALUE]
        .into_iter()
        .filter(|s| contains(bytes, s.as_bytes()))
        .collect();
    for (name, n) in [(ITEM_VALUE, 61.8125f64), (SCORE_VALUE, 73.53125f64)] {
        if contains(bytes, &n.to_be_bytes()) && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if e.file_type().unwrap().is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Every sentinel `bytes` holds, as text, and each number also as the
/// 8-byte big-endian double SQLite stores.
fn found(bytes: &[u8]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = SENTINELS
        .iter()
        .chain([TITLE].iter())
        .copied()
        .filter(|s| contains(bytes, s.as_bytes()))
        .collect();
    for (name, n) in [(SEL_NUM, 61.8129f64), (UNSEL_NUM, 7123.4567f64)] {
        if contains(bytes, &n.to_be_bytes()) && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Every file under the app's data folder but the project's own, by its
/// path, with its SHA-256.
fn the_rest(app: &Path, project: &Path) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    files_under(app, &mut files);
    files
        .iter()
        .filter(|f| !f.starts_with(project))
        .map(|f| {
            (
                f.strip_prefix(app).unwrap().to_string_lossy().into_owned(),
                gaply_core::contracts::ContentHash::of_bytes(&std::fs::read(f).unwrap())
                    .as_hex()
                    .to_string(),
            )
        })
        .collect()
}

#[test]
fn a_rerun_puts_no_sentinel_anywhere_but_the_projects_artifacts_and_its_title() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();

    let w = World::new("rerun-matrix");
    let app = w.root();
    std::fs::create_dir_all(&app).unwrap();
    let _db = Database::open(&app.join("gaply.db")).expect("a migrated database");
    let folder = w.dir.0.join(PATH);
    std::fs::create_dir_all(&folder).unwrap();
    let data = folder.join(format!("{FILE}.csv"));
    std::fs::write(&data, table()).unwrap();

    let mut responses: Vec<(String, String)> = Vec::new();
    let e = w.engine();
    let opened = desc::open(&e.datasets, &data).expect("open");
    let held = e.datasets.get(opened.slot).unwrap();
    let spec = stats_desc::spec(
        OperationId(1),
        held.dataset(),
        DescribeParams {
            columns: vec![
                ColumnRequest {
                    alias: Alias(ColumnAlias(1)),
                    treat_as: TreatAs::Numeric,
                },
                ColumnRequest {
                    alias: Alias(ColumnAlias(2)),
                    treat_as: TreatAs::Categorical,
                },
            ],
            missing_codes: vec![],
        },
    );
    drop(held);
    let run = desc::run(&e.runs, &e.datasets, opened.slot, spec)
        .unwrap()
        .run_id;
    w.wait(run);
    let saved = pc::save_run(
        &app,
        &w.ws,
        run,
        SaveTarget::NewProject {
            name: TITLE.to_string(),
        },
    )
    .expect("saved");
    let id = saved.project_id.clone();
    let project = app.join("projects").join(&id);
    let before = the_rest(&app, &project);
    assert!(before.contains_key("gaply.db"), "{before:?}");

    let started = pc::rerun(&app, &w.ws, 1).expect("the rerun starts");
    responses.push(("rerun".into(), serde_json::to_string(&started).unwrap()));
    let start = std::time::Instant::now();
    let current = loop {
        let c = pc::current(&w.ws).unwrap();
        if c.rerun
            .as_ref()
            .is_some_and(|r| r.status != RerunProgress::Going)
        {
            break c;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(20));
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(current.rerun.as_ref().unwrap().status, RerunProgress::Saved);
    responses.push(("current".into(), serde_json::to_string(&current).unwrap()));

    // Phase 8G-2 (decision 330): a one-way ANOVA, saved into the open
    // project and run again, the same way.
    let held = e.datasets.get(opened.slot).unwrap();
    let anova_spec = stats_anova::spec(
        OperationId(2),
        held.dataset(),
        AnovaParams {
            outcome: Alias(ColumnAlias(1)),
            group: Alias(ColumnAlias(2)),
            levels: None,
            post_hoc: vec![PostHoc::Tukey, PostHoc::GamesHowell, PostHoc::Bonferroni],
            missing_codes: vec![],
        },
    );
    drop(held);
    let anova_run =
        app_lib::stats_anova_commands::run(&e.runs, &e.datasets, opened.slot, anova_spec)
            .unwrap()
            .run_id;
    w.wait_new_kind(anova_run);
    let saved_anova = pc::save_run(
        &app,
        &w.ws,
        anova_run,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .expect("the ANOVA saved");
    responses.push((
        "save (ANOVA)".into(),
        serde_json::to_string(&saved_anova).unwrap(),
    ));
    let started = pc::rerun(&app, &w.ws, 3).expect("the ANOVA's rerun starts");
    responses.push((
        "rerun (ANOVA)".into(),
        serde_json::to_string(&started).unwrap(),
    ));
    let start = std::time::Instant::now();
    let current = loop {
        let c = pc::current(&w.ws).unwrap();
        if c.rerun
            .as_ref()
            .is_some_and(|r| r.status != RerunProgress::Going && r.stored_run_id == Some(4))
        {
            break c;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(20));
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(current.rerun.as_ref().unwrap().status, RerunProgress::Saved);
    responses.push((
        "current (ANOVA)".into(),
        serde_json::to_string(&current).unwrap(),
    ));

    // Phase 8H-2 (decision 357): correlations, saved into the open project
    // and run again, the same way.
    let held = e.datasets.get(opened.slot).unwrap();
    let corr_spec = stats_corr::spec(
        OperationId(3),
        held.dataset(),
        CorrParams {
            variables: vec![Alias(ColumnAlias(1)), Alias(ColumnAlias(4))],
            coefficients: vec![
                Coefficient::Pearson,
                Coefficient::Spearman,
                Coefficient::Kendall,
            ],
            missing: MissingHandling::Pairwise,
            missing_codes: vec![],
        },
    );
    drop(held);
    let corr_run = app_lib::stats_corr_commands::run(&e.runs, &e.datasets, opened.slot, corr_spec)
        .unwrap()
        .run_id;
    w.wait_new_kind(corr_run);
    let saved_corr = pc::save_run(
        &app,
        &w.ws,
        corr_run,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .expect("the correlations saved");
    responses.push((
        "save (correlations)".into(),
        serde_json::to_string(&saved_corr).unwrap(),
    ));
    let started = pc::rerun(&app, &w.ws, 5).expect("the correlations' rerun starts");
    responses.push((
        "rerun (correlations)".into(),
        serde_json::to_string(&started).unwrap(),
    ));
    let start = std::time::Instant::now();
    let current = loop {
        let c = pc::current(&w.ws).unwrap();
        if c.rerun
            .as_ref()
            .is_some_and(|r| r.status != RerunProgress::Going && r.stored_run_id == Some(6))
        {
            break c;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(20));
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(current.rerun.as_ref().unwrap().status, RerunProgress::Saved);
    responses.push((
        "current (correlations)".into(),
        serde_json::to_string(&current).unwrap(),
    ));

    // Phase 8I-2 (decision 382): Cronbach's alpha of the selected column and
    // `aux`, saved into the open project and run again, the same way.
    let held = e.datasets.get(opened.slot).unwrap();
    let alpha_spec = stats_alpha::spec(
        OperationId(4),
        held.dataset(),
        AlphaParams {
            items: vec![Alias(ColumnAlias(1)), Alias(ColumnAlias(4))],
            reversed: vec![],
            scale: None,
            missing_codes: vec![],
        },
    );
    drop(held);
    let alpha_run =
        app_lib::stats_alpha_commands::run(&e.runs, &e.datasets, opened.slot, alpha_spec)
            .unwrap()
            .run_id;
    w.wait_new_kind(alpha_run);
    let saved_alpha = pc::save_run(
        &app,
        &w.ws,
        alpha_run,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .expect("alpha saved");
    responses.push((
        "save (alpha)".into(),
        serde_json::to_string(&saved_alpha).unwrap(),
    ));
    let started = pc::rerun(&app, &w.ws, 7).expect("alpha's rerun starts");
    responses.push((
        "rerun (alpha)".into(),
        serde_json::to_string(&started).unwrap(),
    ));
    let start = std::time::Instant::now();
    let current = loop {
        let c = pc::current(&w.ws).unwrap();
        if c.rerun
            .as_ref()
            .is_some_and(|r| r.status != RerunProgress::Going && r.stored_run_id == Some(8))
        {
            break c;
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(20));
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert_eq!(current.rerun.as_ref().unwrap().status, RerunProgress::Saved);
    responses.push((
        "current (alpha)".into(),
        serde_json::to_string(&current).unwrap(),
    ));

    // Phase 8M-4 (decision 437): a summary and Cronbach's alpha of a score, on a
    // table of their own, saved into the open project and run again.
    let prepared_data = folder.join(format!("{FILE}_prepared.csv"));
    std::fs::write(&prepared_data, prepared_table()).unwrap();
    let prepared_open = desc::open(&e.datasets, &prepared_data).expect("open");
    let recipe = prepared_recipe(7, NAME);
    let made = prep::make(&e.runs, &e.datasets, prepared_open.slot, &recipe, 1_000_000)
        .map_err(|_| "the make")
        .unwrap();
    let view = serde_json::to_string(&made.dataset.as_ref().expect("the view")).unwrap();
    assert_eq!(prepared_found(view.as_bytes())[0], NAME, "the screen's own");
    let mut prepared_surfaces: Vec<(String, String)> = Vec::new();
    let mut prepared_results = String::new();
    for (stored, run) in [
        (9, summary_of(&w, prepared_open.slot, 8)),
        (11, alpha_of(&w, prepared_open.slot)),
    ] {
        let saved = pc::save_run(
            &app,
            &w.ws,
            run,
            SaveTarget::OpenProject {
                project_id: id.clone(),
            },
        )
        .expect("the prepared run saved");
        assert_eq!(saved.stored_run_id, stored);
        prepared_surfaces.push((
            format!("save {stored}"),
            serde_json::to_string(&saved).unwrap(),
        ));
        prepared_surfaces.push((format!("rerun {stored}"), rerun_saved(&w, &app, stored)));
        for n in [stored, stored + 1] {
            let back = pc::run_result(&app, &w.ws, &id, n).unwrap();
            prepared_results.push_str(&serde_json::to_string(&back).unwrap());
        }
    }
    // Phase 8T-1a (decision 491): a run keeps its recipe, so a Save after a
    // second make succeeds, as stored run 13, with the recipe it ran on: NAME's,
    // reused by its hash, so the files swept below are the same two.
    let kept = summary_of(&w, prepared_open.slot, 8);
    prep::make(
        &e.runs,
        &e.datasets,
        prepared_open.slot,
        &prepared_recipe(7, "another"),
        1_000_000,
    )
    .map_err(|_| "the second make")
    .unwrap();
    let saved = pc::save_run(
        &app,
        &w.ws,
        kept,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .expect("a Save after a second make keeps the run's own recipe");
    assert_eq!(saved.stored_run_id, 13);
    prepared_surfaces.push((
        "a Save after a second make".into(),
        serde_json::to_string(&saved).unwrap(),
    ));
    // A Save refused by another route: the file closed since the run.
    let unsaved = summary_of(&w, prepared_open.slot, 8);
    assert!(desc::close(&e.datasets, prepared_open.slot));
    let refused = pc::save_run(
        &app,
        &w.ws,
        unsaved,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .map(|_| ())
    .unwrap_err();
    prepared_surfaces.push(("a refused Save".into(), refused.to_string()));
    assert!(refused.to_string().contains("project.data_unavailable"));

    // Phase 8X-2 (decision 590): crosstabs of the two text columns of a table
    // of its own, the code -99 ticked, named and placed as the first file is,
    // saved into the open project as run 14 and run again as 15. Placed after
    // the prepared runs, so no stored run id above moves.
    let xtab_data = folder.join(format!("{FILE}_xtab.csv"));
    std::fs::write(&xtab_data, xtab_table()).unwrap();
    let xtab_open = desc::open(&e.datasets, &xtab_data).expect("open");
    let held = e.datasets.get(xtab_open.slot).unwrap();
    let xtab_spec = stats_xtab::spec(
        OperationId(7),
        held.dataset(),
        CrosstabsParams {
            rows: Alias(ColumnAlias(1)),
            columns: Alias(ColumnAlias(2)),
            missing_codes: vec![-99.0],
        },
    );
    drop(held);
    let xtab_run = app_lib::stats_xtab_commands::run(&e.runs, &e.datasets, xtab_open.slot, xtab_spec)
        .unwrap()
        .run_id;
    w.wait_new_kind(xtab_run);
    let saved_xtab = pc::save_run(
        &app,
        &w.ws,
        xtab_run,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .expect("the crosstab saved");
    assert_eq!(saved_xtab.stored_run_id, 14);
    responses.push((
        "save (crosstabs)".into(),
        serde_json::to_string(&saved_xtab).unwrap(),
    ));
    responses.push(("rerun (crosstabs)".into(), rerun_saved(&w, &app, 14)));

    // Phase 8Y-2 (decision 621): the rank tests on table(), through the slot
    // the ANOVA used, saved into the open project as run 16 and run again as
    // 17. After crosstabs', so no stored run id above moves.
    let held = e.datasets.get(opened.slot).unwrap();
    let ranks_spec = stats_ranks::spec(
        OperationId(8),
        held.dataset(),
        IndependentRanksParams {
            outcome: Alias(ColumnAlias(1)),
            group: Alias(ColumnAlias(2)),
            levels: None,
            missing_codes: vec![],
        },
    );
    drop(held);
    let ranks_run =
        app_lib::stats_ranks_commands::run(&e.runs, &e.datasets, opened.slot, ranks_spec)
            .unwrap()
            .run_id;
    w.wait_new_kind(ranks_run);
    let saved_ranks = pc::save_run(
        &app,
        &w.ws,
        ranks_run,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .expect("the rank test saved");
    assert_eq!(saved_ranks.stored_run_id, 16);
    responses.push((
        "save (rank tests)".into(),
        serde_json::to_string(&saved_ranks).unwrap(),
    ));
    responses.push(("rerun (rank tests)".into(), rerun_saved(&w, &app, 16)));
    // Phase 8Z-1 (decision 635): a positive control. Run 16 ran, is the
    // Mann-Whitney test of table()'s two levels and holds LVL; its rerun ran.
    let read_16 = pc::run_result(&app, &w.ws, &id, 16).unwrap();
    assert_eq!(read_16.state, RunState::Succeeded, "run 16 succeeded");
    let bundle_16: serde_json::Value =
        serde_json::from_str(read_16.bundle_json.as_deref().expect("a result")).unwrap();
    assert!(
        bundle_16["primaryOutputs"]["test"]["mann_whitney"].is_object(),
        "run 16 is a Mann-Whitney result"
    );
    assert!(
        read_16.bundle_json.as_deref().unwrap().contains(LVL),
        "run 16's result holds its level label"
    );
    assert_eq!(
        pc::run_result(&app, &w.ws, &id, 17).unwrap().state,
        RunState::Succeeded,
        "run 17 succeeded"
    );

    // Phase 8Z-2 (decision 655): the related rank tests. After the rank tests', so no
    // stored run id above moves. Run 18, a Wilcoxon test of table()'s C1 and C4, through the
    // slot the rank tests used, and run 19 its rerun.
    let held = e.datasets.get(opened.slot).unwrap();
    let wilcoxon_spec = stats_related_ranks::spec(
        OperationId(18),
        held.dataset(),
        RelatedRanksParams {
            columns: vec![Alias(ColumnAlias(1)), Alias(ColumnAlias(4))],
            missing_codes: vec![],
        },
    );
    drop(held);
    let wilcoxon_run =
        app_lib::stats_related_ranks_commands::run(&e.runs, &e.datasets, opened.slot, wilcoxon_spec)
            .unwrap()
            .run_id;
    w.wait_new_kind(wilcoxon_run);
    let saved_wilcoxon = pc::save_run(
        &app,
        &w.ws,
        wilcoxon_run,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .expect("the Wilcoxon test saved");
    assert_eq!(saved_wilcoxon.stored_run_id, 18);
    responses.push((
        "save (Wilcoxon)".into(),
        serde_json::to_string(&saved_wilcoxon).unwrap(),
    ));
    responses.push(("rerun (Wilcoxon)".into(), rerun_saved(&w, &app, 18)));

    // Run 20, a Friedman test on a table of its own, named and placed as the first file
    // is, and run 21 its rerun.
    let friedman_data = folder.join(format!("{FILE}_friedman.csv"));
    std::fs::write(&friedman_data, friedman_table()).unwrap();
    let friedman_open = desc::open(&e.datasets, &friedman_data).expect("open");
    let held = e.datasets.get(friedman_open.slot).unwrap();
    let friedman_spec = stats_related_ranks::spec(
        OperationId(20),
        held.dataset(),
        RelatedRanksParams {
            columns: vec![Alias(ColumnAlias(1)), Alias(ColumnAlias(2)), Alias(ColumnAlias(3))],
            missing_codes: vec![],
        },
    );
    drop(held);
    let friedman_run = app_lib::stats_related_ranks_commands::run(
        &e.runs,
        &e.datasets,
        friedman_open.slot,
        friedman_spec,
    )
    .unwrap()
    .run_id;
    w.wait_new_kind(friedman_run);
    let saved_friedman = pc::save_run(
        &app,
        &w.ws,
        friedman_run,
        SaveTarget::OpenProject {
            project_id: id.clone(),
        },
    )
    .expect("the Friedman test saved");
    assert_eq!(saved_friedman.stored_run_id, 20);
    responses.push((
        "save (Friedman)".into(),
        serde_json::to_string(&saved_friedman).unwrap(),
    ));
    responses.push(("rerun (Friedman)".into(), rerun_saved(&w, &app, 20)));
    // Positive controls: each ran, and each is the test it should be.
    let read_18 = pc::run_result(&app, &w.ws, &id, 18).unwrap();
    assert_eq!(read_18.state, RunState::Succeeded, "run 18 succeeded");
    let bundle_18: serde_json::Value =
        serde_json::from_str(read_18.bundle_json.as_deref().expect("a result")).unwrap();
    assert!(
        bundle_18["primaryOutputs"]["test"]["wilcoxon"].is_object(),
        "run 18 is a Wilcoxon result"
    );
    assert_eq!(
        bundle_18["primaryOutputs"]["columns"].as_array().unwrap().len(),
        2,
        "run 18 compares two columns"
    );
    let read_20 = pc::run_result(&app, &w.ws, &id, 20).unwrap();
    assert_eq!(read_20.state, RunState::Succeeded, "run 20 succeeded");
    let bundle_20: serde_json::Value =
        serde_json::from_str(read_20.bundle_json.as_deref().expect("a result")).unwrap();
    assert!(
        bundle_20["primaryOutputs"]["test"]["friedman"].is_object(),
        "run 20 is a Friedman result"
    );
    assert_eq!(
        bundle_20["primaryOutputs"]["columns"].as_array().unwrap().len(),
        3,
        "run 20 compares three columns"
    );
    for n in [19, 21] {
        assert_eq!(
            pc::run_result(&app, &w.ws, &id, n).unwrap().state,
            RunState::Succeeded,
            "run {n} succeeded"
        );
    }

    // Phase 9A-2 (decision 686): the linear regression. After the related rank tests', so no
    // stored run id above moves. Run 22, C1 on [C4] of table(), through the slot the rank
    // tests used, and run 23 its rerun; run 24, C1 on [C3, C2] of friedman_table(), through
    // the Friedman test's slot, still held from runs 20 and 21, and run 25 its rerun. The
    // phase's report computes both models by hand: no number either holds, as JSON prints
    // it, contains a sentinel's text.
    for (n, slot, outcome, predictors) in [
        (22u64, opened.slot, 1u32, vec![4u32]),
        (24, friedman_open.slot, 1, vec![3, 2]),
    ] {
        let held = e.datasets.get(slot).unwrap();
        let spec = stats_linear_regression::spec(
            OperationId(n),
            held.dataset(),
            LinearRegressionParams {
                outcome: Alias(ColumnAlias(outcome)),
                predictors: predictors.iter().map(|c| Alias(ColumnAlias(*c))).collect(),
                missing_codes: vec![],
            },
        );
        drop(held);
        let run = app_lib::stats_linear_regression_commands::run(&e.runs, &e.datasets, slot, spec)
            .unwrap()
            .run_id;
        w.wait_new_kind(run);
        let saved = pc::save_run(
            &app,
            &w.ws,
            run,
            SaveTarget::OpenProject {
                project_id: id.clone(),
            },
        )
        .expect("the regression saved");
        assert_eq!(saved.stored_run_id, n);
        responses.push((
            format!("save (regression {n})"),
            serde_json::to_string(&saved).unwrap(),
        ));
        responses.push((format!("rerun (regression {n})"), rerun_saved(&w, &app, n)));
    }
    // Positive controls: each ran, with one and two predictors respectively, and each
    // rerun ran.
    for (n, k) in [(22, 1), (24, 2)] {
        let read = pc::run_result(&app, &w.ws, &id, n).unwrap();
        assert_eq!(read.state, RunState::Succeeded, "run {n} succeeded");
        let bundle: serde_json::Value =
            serde_json::from_str(read.bundle_json.as_deref().expect("a result")).unwrap();
        assert_eq!(
            bundle["primaryOutputs"]["predictors"].as_array().unwrap().len(),
            k,
            "run {n} has {k} predictor(s)"
        );
        assert_eq!(
            pc::run_result(&app, &w.ws, &id, n + 1).unwrap().state,
            RunState::Succeeded,
            "run {} succeeded",
            n + 1
        );
    }

    // Phase 9B-3 (decision 731): the factor analysis. After the regressions', so no stored
    // run id above moves. table() has two numeric columns, so both runs read C1, C2 and C3
    // of friedman_table() through the Friedman test's slot, still held; aux (C4) stays
    // unchosen. Run 26: principal components under Kaiser's rule with varimax, and run 27
    // its rerun; run 28: principal axis with two factors and promax, and run 29 its rerun.
    for (n, extraction, factors, rotation) in [
        (26u64, Extraction::PrincipalComponents, None, Rotation::Varimax),
        (28, Extraction::PrincipalAxis, Some(2u32), Rotation::Promax),
    ] {
        let held = e.datasets.get(friedman_open.slot).unwrap();
        let spec = stats_factor::spec(
            OperationId(n),
            held.dataset(),
            FactorParams {
                items: (1..=3).map(|c| Alias(ColumnAlias(c))).collect(),
                extraction,
                factors,
                rotation,
                iterations: 25,
                convergence: 0.001,
                missing_codes: vec![],
            },
        );
        drop(held);
        let run = app_lib::stats_factor_commands::run(&e.runs, &e.datasets, friedman_open.slot, spec)
            .unwrap()
            .run_id;
        w.wait_new_kind(run);
        let saved = pc::save_run(
            &app,
            &w.ws,
            run,
            SaveTarget::OpenProject {
                project_id: id.clone(),
            },
        )
        .expect("the factor analysis saved");
        assert_eq!(saved.stored_run_id, n);
        responses.push((
            format!("save (factor {n})"),
            serde_json::to_string(&saved).unwrap(),
        ));
        responses.push((format!("rerun (factor {n})"), rerun_saved(&w, &app, n)));
    }
    // Positive controls: each ran, on three items, with its rotation given, and each rerun
    // ran.
    for (n, method) in [(26, "varimax"), (28, "promax")] {
        let read = pc::run_result(&app, &w.ws, &id, n).unwrap();
        assert_eq!(read.state, RunState::Succeeded, "run {n} succeeded");
        let bundle: serde_json::Value =
            serde_json::from_str(read.bundle_json.as_deref().expect("a result")).unwrap();
        assert_eq!(
            bundle["primaryOutputs"]["items"].as_array().unwrap().len(),
            3,
            "run {n} has three items"
        );
        assert_eq!(bundle["primaryOutputs"]["rotation"]["method"], method, "run {n} is rotated");
        assert_eq!(
            pc::run_result(&app, &w.ws, &id, n + 1).unwrap().state,
            RunState::Succeeded,
            "run {} succeeded",
            n + 1
        );
    }

    let runs_response = serde_json::to_string(&pc::runs(&app, &w.ws, &id).unwrap()).unwrap();
    responses.push(("runs".into(), runs_response.clone()));
    // Phase 8U-3 (decision 553): project_runs holds no name, nor any other of
    // the prepared run's sentinels.
    prepared_surfaces.push(("runs".into(), runs_response));
    let mut results = String::new();
    // Phase 8X-2 (decision 590): the crosstab and its rerun, 14 and 15; Phase
    // 8Y-2 (decision 621): the rank test and its rerun, 16 and 17; Phase 8Z-2 (decision
    // 655): the related rank tests and their reruns, 18 to 21; Phase 9A-2 (decision 686): the
    // regressions and their reruns, 22 to 25; Phase 9B-3 (decision 731): the factor analyses
    // and their reruns, 26 to 29.
    for n in [1, 2, 3, 4, 5, 6, 7, 8, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29] {
        results.push_str(
            &serde_json::to_string(&pc::run_result(&app, &w.ws, &id, n).unwrap()).unwrap(),
        );
    }
    let report = pc::report(&app, &w.ws, &id).unwrap();
    let prov = pc::provenance(&app, &w.ws, &id).unwrap();
    let report = String::from_utf8(report.bytes).unwrap();
    let prov = String::from_utf8(prov.bytes).unwrap();
    assert!(
        report.contains("Repeats run 1."),
        "the rerun is in the report"
    );
    assert!(
        report.contains("Repeats run 3.") && report.contains("- Post hoc tests: tukey"),
        "the ANOVA and its rerun are in the report"
    );
    assert!(
        report.contains("Repeats run 5.")
            && report.contains("- Coefficients: pearson, spearman, kendall"),
        "the correlations and their rerun are in the report"
    );
    assert!(
        report.contains("Repeats run 7.") && report.contains("- Items: columns 1, 4, by position"),
        "alpha and its rerun are in the report"
    );
    // Phase 8X-2 (decision 590): the crosstab and its rerun, its two columns by
    // position and role.
    assert!(
        report.contains("Repeats run 14.")
            && report.matches("- Column 1: table_rows").count() == 2
            && report.matches("- Column 2: table_columns").count() == 2,
        "the crosstab and its rerun are in the report"
    );
    // Phase 8Y-2 (decision 621): the rank test and its rerun.
    assert!(
        report.contains("Repeats run 16."),
        "the rank test and its rerun are in the report"
    );
    // Phase 8Z-2 (decision 655): the related rank tests and their reruns, each's columns
    // by position in the order chosen.
    assert!(report.contains("Repeats run 18."), "the Wilcoxon test's rerun is in the report");
    assert!(report.contains("Repeats run 20."), "the Friedman test's rerun is in the report");
    for line in [
        "- Columns compared: columns 1, 4, by position, in the order chosen",
        "- Columns compared: columns 1, 2, 3, by position, in the order chosen",
    ] {
        assert_eq!(report.matches(line).count(), 2, "{line}");
    }
    // Phase 9A-2 (decision 686): the regressions and their reruns, counted by their
    // Predictors lines, which no other method writes ("- Column 1: outcome" is the t-test's
    // and the rank tests' too).
    assert!(report.contains("Repeats run 22."), "the first regression's rerun is in the report");
    assert!(report.contains("Repeats run 24."), "the second regression's rerun is in the report");
    for line in [
        "- Predictors: column 4, by position",
        "- Predictors: columns 3, 2, by position, in the order chosen",
    ] {
        assert_eq!(report.matches(line).count(), 2, "{line}");
    }
    // Phase 9B-3 (decision 731): the factor analyses and their reruns, counted by their own
    // lines ("- Items:" is alpha's too).
    assert!(report.contains("Repeats run 26."), "the first factor analysis's rerun is in the report");
    assert!(report.contains("Repeats run 28."), "the second factor analysis's rerun is in the report");
    for line in [
        "- Extraction: principal components",
        "- Extraction: principal axis factoring",
        "- Rotation: varimax",
        "- Rotation: promax",
        "- Factors: every eigenvalue above 1",
        "- Factors: 2",
    ] {
        assert_eq!(report.matches(line).count(), 2, "{line}");
    }
    assert!(prov.contains("wasInformedBy"), "and in the provenance");

    // Phase 8M-4 (decision 437): the prepared run is in both exports, by
    // position, and none of its three sentinels is in anything but its two files.
    assert!(
        report.contains("- Prepared column 8: mean of columns 1 and 2;"),
        "the prepared run is in the report"
    );
    assert!(prov.contains("gaply:recipe"), "and in the provenance");
    prepared_surfaces.push(("the report".into(), report.clone()));
    prepared_surfaces.push(("the provenance".into(), prov.clone()));
    prepared_surfaces.push(("results read back".into(), prepared_results.clone()));
    // Phase 8U-3 (decision 553): project_recipes' response is the one project
    // read that holds the name, in memory, for the page, as the DatasetView
    // does above; read before the recipe is damaged below, and holding no
    // item's value and no score's value.
    let recipes = serde_json::to_string(&pc::recipes(&app, &w.ws, &id).unwrap()).unwrap();
    assert_eq!(
        prepared_found(recipes.as_bytes()),
        [NAME],
        "project_recipes' response: the name and nothing else"
    );
    assert_eq!(found(recipes.as_bytes()), Vec::<&str>::new(), "project_recipes");
    // A rerun refused: its recipe damaged on disk, the name still in the file.
    let recipe_hash = stats_prep::recipe_hash(&recipe);
    let recipe_path = project.join("artifacts").join(&recipe_hash);
    let mut damaged = std::fs::read(&recipe_path).unwrap();
    damaged[0] ^= 1;
    let beside = recipe_path.with_extension("damaged");
    std::fs::write(&beside, &damaged).unwrap();
    std::fs::rename(&beside, &recipe_path).unwrap();
    let refused = pc::rerun(&app, &w.ws, 9).map(|_| ()).unwrap_err();
    assert!(refused.to_string().contains("project.artifact_wrong_hash"));
    prepared_surfaces.push(("a refused rerun".into(), refused.to_string()));
    for (name, text) in &prepared_surfaces {
        assert_eq!(
            prepared_found(text.as_bytes()),
            Vec::<&str>::new(),
            "{name}"
        );
    }

    // Responses: none, but a result read back (the header and the label) and
    // the report's title (the name).
    for (name, text) in &responses {
        assert_eq!(found(text.as_bytes()), Vec::<&str>::new(), "{name}");
    }
    let in_results = found(results.as_bytes());
    assert!(in_results.contains(&HDR) && in_results.contains(&LVL));
    // Phase 8X-2 (decision 590): the crosstab's second level label, a level
    // label in a result, as LVL is; never anywhere else.
    assert!(in_results.contains(&LV2), "the crosstab's result holds its labels");
    for s in &in_results {
        assert!([HDR, LVL, LV2].contains(s), "{s} in a result read back");
    }
    assert_eq!(found(report.as_bytes()), [TITLE], "the report's title only");
    assert_eq!(report.matches(TITLE).count(), 1);
    assert_eq!(found(prov.as_bytes()), Vec::<&str>::new(), "the provenance");

    // Files: the data's sentinels only under artifacts/, the name only in
    // project.db, the rest of the folder as it was.
    pc::close(&app, &w.ws).unwrap();
    let artifacts = project.join("artifacts");
    let mut files = Vec::new();
    files_under(&app, &mut files);
    let (mut in_artifacts, mut in_db) = (Vec::new(), Vec::new());
    // Phase 8M-4 (decision 437): the name only in the recipe's file, an item's
    // value only in the data file's, and the score's value in no file at all,
    // project.db's bytes and its -wal included.
    let prepared_file = artifacts.join(
        gaply_core::contracts::ContentHash::of_bytes(&prepared_table())
            .as_hex()
            .to_string(),
    );
    let mut prepared_files_seen = 0;
    for f in &files {
        let hits = prepared_found(&std::fs::read(f).unwrap());
        if *f == recipe_path {
            assert_eq!(hits, [NAME], "the recipe's file");
            prepared_files_seen += 1;
        } else if *f == prepared_file {
            assert_eq!(hits, [ITEM_VALUE], "the data file's copy");
            prepared_files_seen += 1;
        } else {
            assert_eq!(hits, Vec::<&str>::new(), "{}", f.display());
        }
    }
    assert_eq!(prepared_files_seen, 2, "each file was swept");
    assert!(
        files.iter().any(|f| f.ends_with("project.db")),
        "project.db was swept"
    );
    for f in &files {
        let hits = found(&std::fs::read(f).unwrap());
        if f.starts_with(&artifacts) {
            in_artifacts.extend(hits.iter().copied());
            assert!(!hits.contains(&TITLE), "the name in {}", f.display());
        } else if f.parent() == Some(project.as_path())
            && f.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("project.db")
        {
            in_db.extend(hits.iter().copied());
            assert!(
                hits.iter().all(|s| *s == TITLE),
                "{hits:?} in {}",
                f.display()
            );
        } else {
            assert!(hits.is_empty(), "{hits:?} in {}", f.display());
        }
        for s in [FILE, PATH] {
            assert!(!hits.contains(&s), "{s} in {}", f.display());
        }
    }
    for s in [HDR, LVL, TXT, SEL_NUM, UNSEL_NUM, LV2] {
        assert!(in_artifacts.contains(&s), "the copy of the data holds {s}");
    }
    assert!(in_db.contains(&TITLE), "the name is in project.title");
    assert_eq!(
        the_rest(&app, &project),
        before,
        "the rest of the app's folder is byte for byte as it was"
    );

    // Tracing: every line, from every thread, the worker's included.
    let traced = lines.lock().unwrap().clone();
    assert!(
        traced.iter().any(|l| l.contains("project: rerun saved")),
        "the save at the end's event was captured: {} lines",
        traced.len()
    );
    for l in &traced {
        assert_eq!(found(l.as_bytes()), Vec::<&str>::new(), "a log line: {l}");
        assert_eq!(
            prepared_found(l.as_bytes()),
            Vec::<&str>::new(),
            "a log line: {l}"
        );
    }

    // Deleted: no file in the folder holds any.
    pc::open(&app, &w.ws, &id).unwrap();
    let c = pc::prepare_delete(&app, &w.ws, &id).unwrap();
    pc::delete(&app, &w.ws, &id, Some(&c.confirmation)).unwrap();
    let mut left = Vec::new();
    files_under(&app, &mut left);
    for f in &left {
        let bytes = std::fs::read(f).unwrap();
        assert!(found(&bytes).is_empty(), "{} after the delete", f.display());
        assert!(
            prepared_found(&bytes).is_empty(),
            "{} after the delete",
            f.display()
        );
    }
    println!(
        "lines: {}; files before the delete: {}; after: {}",
        traced.len(),
        files.len(),
        left.len()
    );
}

/// The prepared run's negative control (decision 437), 8M-3's in a project: a
/// score constant at the sentinel, whose mean the summary's result shows, so
/// here its value IS in the bundle and the result read back, and the search can
/// find what it seeks. Still not in `project.db`, and the name in neither.
///
/// It works in a project of its own with its own session; its tracing lines
/// reach the subscriber the test above owns, and hold no sentinel of either.
#[test]
fn a_saved_summary_holds_the_value_when_a_score_is_constant_at_it() {
    let w = World::new("rerun-matrix-control");
    let app = w.root();
    let rows: Vec<Vec<C>> = (0..6)
        .map(|_| vec![C::N(ITEM_VALUE), C::N("85.25")])
        .collect();
    let path = w
        .dir
        .write("control.csv", &delimited(&["q1", "q2"], &rows, ",", "\n"));
    let e = w.engine();
    let opened = desc::open(&e.datasets, &path).expect("open");
    prep::make(
        &e.runs,
        &e.datasets,
        opened.slot,
        &prepared_recipe(2, NAME),
        1_000,
    )
    .map_err(|_| "the make")
    .unwrap();
    let run = summary_of(&w, opened.slot, 3);
    let saved = pc::save_run(
        &app,
        &w.ws,
        run,
        SaveTarget::NewProject {
            name: "Control".to_string(),
        },
    )
    .expect("saved");
    let back = pc::run_result(&app, &w.ws, &saved.project_id, 1).unwrap();
    let read_back = serde_json::to_string(&back).unwrap();
    assert!(
        contains(read_back.as_bytes(), SCORE_VALUE.as_bytes()),
        "73.53125 is every row's score, so the result read back must show it"
    );
    assert!(!contains(read_back.as_bytes(), NAME.as_bytes()));
    pc::close(&app, &w.ws).unwrap();

    let project = app.join("projects").join(&saved.project_id);
    let mut files = Vec::new();
    files_under(&project, &mut files);
    let mut bundles = 0;
    for f in &files {
        let bytes = std::fs::read(f).unwrap();
        if contains(&bytes, b"primaryOutputs") {
            bundles += 1;
            assert!(
                contains(&bytes, SCORE_VALUE.as_bytes()),
                "the saved bundle holds it"
            );
            assert!(!contains(&bytes, NAME.as_bytes()), "and no name");
        }
        if f.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("project.db")
        {
            assert_eq!(prepared_found(&bytes), Vec::<&str>::new());
        }
    }
    assert_eq!(bundles, 1);
}

/// The matrix's negative control (Research OS, Phase 8H, decision 337d): on
/// a table of its own in which `61.8129` IS its group's maximum (95.5 moved
/// to the other group, so the label's group is 10.5, 61.8129, 50 and 31),
/// the one-way ANOVA's result holds it, as each group's min and max. With
/// the matrix's own pass, this shows that the 8G-2 placement, not anything
/// else, is what keeps the value out of the ANOVA's saved result.
///
/// It calls `stats_anova::compute` directly, with no command, no project and
/// no tracing of its own: the test above owns the process's subscriber.
/// `build_dataset` logs only its two counts.
#[test]
fn the_anova_result_holds_the_value_when_it_is_its_groups_maximum() {
    use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};
    let score = [
        "44.1257", "10.5", "95.5", SEL_NUM, "23.25", "50", "82", "31", "70.75",
    ];
    let group = ["a", LVL, "a", LVL, "a", LVL, "a", LVL, "a"];
    let rows: Vec<Vec<String>> = (0..9)
        .map(|i| vec![score[i].to_string(), group[i].to_string()])
        .collect();
    let d = build_dataset(
        RawTable::from_rows(vec![HDR.into(), "group".into()], rows),
        DataFingerprint::of_bytes(b"negative control", "test", TableRef::FirstTable),
    )
    .expect("dataset");
    let alias = |i: usize| Alias(d.schema.columns[i].alias);
    let result = stats_anova::compute(
        &d,
        &AnovaParams {
            outcome: alias(0),
            group: alias(1),
            levels: None,
            post_hoc: Vec::new(),
            missing_codes: Vec::new(),
        },
    )
    .expect("the ANOVA computes");
    let bytes = serde_json::to_vec(&result).unwrap();
    assert!(
        found(&bytes).contains(&SEL_NUM),
        "61.8129 is its group's maximum here, so the result must hold it"
    );
}
