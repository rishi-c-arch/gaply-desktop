//! **Scale scores' privacy matrix (Research OS, Phase 8M-3, decision 427;
//! ADR-011 Part 11), in the pattern of `stats_alpha_privacy_matrix.rs`.** A name
//! sentinel, an item value sentinel and a score value sentinel, with the file's
//! own, over every surface.
//!
//! Allowed: the typed name, and any value, in `DatasetView`; the name in the
//! make command's recipe in its request and in the read command's recipe in its
//! response; all three in the prepared CSV. Forbidden everywhere else: the spec,
//! every result, manifest and result CSV, every refusal, every tracing span and
//! event (a global subscriber, so the worker's and `spawn_blocking`'s are
//! covered) and every file in the app-data folder, the database's included,
//! which is SHA-256-identical before and after.
//!
//! The values are on a binary grid, so a score's mean is exact. 61.8125 is an
//! item's value in one row; the score there, the mean of it and 85.25, is
//! 73.53125. Neither is any statistic, and neither is the smallest or largest
//! in its column or in any group, which a summary and the ANOVA print: the matrix itself is the check, and the first
//! assertion on the CSV is what shows they exist to be found.
//!
//! The negative control (decision 337d's lesson) is the second test: a score
//! constant at the sentinel, whose mean the summary's result shows, so the
//! search can find what it seeks and the rule above is not vacuous.
//!
//! Alone in its binary on purpose: its tracing subscriber is the global default
//! for the whole process.

mod desc_support;
mod l1_support;
mod prep_support;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use desc_support::{call, delimited, TempDir, C};
use gaply_core::Database;
use prep_support::{app_with, make, recipe, eight, start, until_terminal};

const NAME: &str = "NAME_SENTINEL_f19";
const HDR: &str = "HDR_SENTINEL_a71";
const FILE: &str = "FILE_SENTINEL_c64";
const PATH: &str = "PATH_SENTINEL_d05";
const ITEM_VALUE: &str = "61.8125";
const SCORE_VALUE: &str = "73.53125";

const SENTINELS: [&str; 6] = [NAME, HDR, FILE, PATH, ITEM_VALUE, SCORE_VALUE];

fn table() -> Vec<u8> {
    let headers = [HDR, "q2", "q3", "q4", "arm", "site", "age"];
    let q1 = [
        "44.5", "10.5", "23.25", ITEM_VALUE, "50", "95.5", "27.75", "31", "70.75", "12", "66",
        "18.5",
    ];
    let q2 = [
        "3.5", "11.25", "2", "85.25", "16", "99", "8.5", "4", "99", "1.5", "6", "2.25",
    ];
    let arm = ["a", "a", "a", "a", "a", "a", "b", "b", "b", "b", "b", "b"];
    let site = ["x", "x", "y", "y", "z", "z", "x", "x", "y", "y", "z", "z"];
    let age = [
        "31", "44", "29", "52", "37", "41", "63", "35", "48", "27", "58", "33",
    ];
    let q6 = ["1", "2", "3", "4", "5", "1", "2", "3", "4", "5", "1", "2"];
    let q7 = ["2", "1", "4", "3", "5", "2", "2", "5", "1", "4", "3", "1"];
    let rows: Vec<Vec<C>> = (0..12)
        .map(|i| {
            vec![
                C::N(q1[i]),
                C::N(q2[i]),
                C::N(q6[i]),
                C::N(q7[i]),
                C::T(arm[i]),
                C::T(site[i]),
                C::N(age[i]),
            ]
        })
        .collect();
    delimited(&headers, &rows, ",", "\n")
}

/// Where a sentinel may appear, by surface.
fn allowed(surface: &str, sentinel: &str) -> bool {
    match surface {
        "DatasetView" => [HDR, FILE, NAME, ITEM_VALUE, SCORE_VALUE].contains(&sentinel),
        "recipe in its request" | "recipe in its response" => sentinel == NAME,
        "prepared CSV" => [HDR, NAME, ITEM_VALUE, SCORE_VALUE].contains(&sentinel),
        _ => false,
    }
}

fn files_under(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn file_hashes(dir: &Path) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    files_under(dir, &mut files);
    files
        .iter()
        .map(|f| {
            (
                f.strip_prefix(dir).unwrap().to_string_lossy().into_owned(),
                gaply_core::contracts::ContentHash::of_bytes(&std::fs::read(f).unwrap())
                    .as_hex()
                    .to_string(),
            )
        })
        .collect()
}

fn bytes_of(export: &Value) -> String {
    let bytes: Vec<u8> = serde_json::from_value(export["bytes"].clone()).unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn no_sentinel_reaches_a_surface_it_is_not_allowed_on() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();

    let root = TempDir::new("prep-matrix");
    let folder = root.0.join(PATH);
    std::fs::create_dir_all(&folder).unwrap();
    let data = folder.join(format!("{FILE}.csv"));
    std::fs::write(&data, table()).unwrap();
    let app_data = root.0.join("appdata");
    std::fs::create_dir_all(&app_data).unwrap();
    let db = Arc::new(Database::open(&app_data.join("gaply.db")).expect("a migrated database"));
    let factory: app_lib::workspace::RunsFactory =
        Arc::new(|| Arc::new(app_lib::runs::Runs::default()));
    let (_app, w) = app_with(&app_data, db, factory);
    call(&w, "workspace_activate", json!({ "id": "statistics" })).expect("activates");
    let before = file_hashes(&app_data);
    assert!(before.keys().any(|k| k == "gaply.db"), "{before:?}");

    let mut surfaces: Vec<(String, String)> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();
    let mut push = |name: &str, text: String| surfaces.push((name.to_string(), text));

    let o = call(
        &w,
        "stats_desc_open",
        json!({ "path": data.to_string_lossy() }),
    )
    .expect("open");
    push("DatasetView", o.to_string());
    let slot = o["slot"].clone();

    // The flow, as a screen would drive it: the mean of the first two items.
    let mean = recipe(7, NAME, "mean", &["C1", "C2"]);
    push("recipe in its request", mean.to_string());
    let made = call(
        &w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": mean }),
    )
    .unwrap();
    assert_eq!(made["status"], "valid", "{made}");
    let view = made["dataset"].clone();
    assert_eq!(
        view["profile"]["columns"][7]["scoreName"], NAME,
        "the screen's own"
    );
    push("DatasetView", view.to_string());
    let mut report = made.clone();
    report.as_object_mut().unwrap().remove("dataset");
    push("PrepMadeView (without its dataset)", report.to_string());

    let read = call(&w, "stats_prep_recipe", json!({ "slot": slot })).unwrap();
    push("recipe in its response", read.to_string());

    let csv = call(&w, "stats_prep_csv", json!({ "slot": slot })).unwrap();
    let csv_text = bytes_of(&csv);
    for s in [NAME, HDR, ITEM_VALUE, SCORE_VALUE] {
        assert!(csv_text.contains(s), "{s} exists to be found: {csv_text}");
    }
    push("prepared CSV", csv_text);
    push(
        "ExportView (prepared CSV), its other fields",
        csv["sha256"].to_string(),
    );

    // Every method, on the score, and the summary on an item as well.
    let methods = eight(&view, "C8");
    for m in &methods {
        push("spec", m.spec.to_string());
        let v = call(
            &w,
            &format!("{}_validate", m.prefix),
            json!({ "slot": slot, "spec": m.spec }),
        )
        .unwrap();
        push("a method's validation view", v.to_string());
        let run = start(&w, &slot, m).unwrap_or_else(|e| panic!("{}: {e}", m.prefix));
        let s = until_terminal(&w, m.prefix, run);
        assert_eq!(s["state"], "succeeded", "{}: {s}", m.prefix);
        push("a method's run state", s.to_string());
        let mut r = call(&w, &format!("{}_result", m.prefix), json!({ "runId": run })).unwrap();
        let manifest = r
            .as_object_mut()
            .unwrap()
            .remove("manifest")
            .expect("a manifest");
        assert!(
            manifest["preparation"].is_object(),
            "{}: {manifest}",
            m.prefix
        );
        push("a method's result (without its manifest)", r.to_string());
        push("manifest (in the result view)", manifest.to_string());
        let e = call(
            &w,
            &format!("{}_export", m.prefix),
            json!({ "runId": run, "format": "csv" }),
        )
        .unwrap();
        push("a method's result CSV", bytes_of(&e));
        let mj = call(
            &w,
            &format!("{}_export", m.prefix),
            json!({ "runId": run, "format": "manifestJson" }),
        )
        .unwrap();
        push("manifest (exported bytes)", bytes_of(&mj));
    }

    // Refusals: each comes back as a closed code.
    let mut bad_name = recipe(7, NAME, "mean", &["C1", "C2"]);
    bad_name["steps"][0]["name"] = json!(HDR);
    let refused = call(
        &w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": bad_name }),
    )
    .unwrap();
    assert_eq!(
        refused["errors"][0]["code"],
        "stats.prep.name_equals_file_header"
    );
    push("a refused make's report", refused.to_string());
    let mut sentinel_name_bad = recipe(7, NAME, "sum", &["C1", "C9"]);
    sentinel_name_bad["steps"][0]["missingCodes"] = json!([-1.0]);
    let refused = call(
        &w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": sentinel_name_bad }),
    )
    .unwrap();
    assert_eq!(refused["status"], "invalid", "{refused}");
    push("a refused make's report", refused.to_string());
    let mut all_codes = recipe(7, NAME, "sum", &["C3", "C4"]);
    all_codes["steps"][0]["missingCodes"] = json!([1.0, 2.0, 3.0, 4.0, 5.0]);
    let refused = call(
        &w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": all_codes }),
    )
    .unwrap();
    assert_eq!(
        refused["errors"][0]["code"], "stats.prep.no_row_scored",
        "{refused}"
    );
    push("a refused make's report", refused.to_string());

    let mut forged = eight(&view, "C8").remove(0);
    forged.spec["preparation"]["recipeHash"] = json!("ab".repeat(32));
    let v = call(
        &w,
        "stats_desc_validate",
        json!({ "slot": slot, "spec": forged.spec }),
    )
    .unwrap();
    assert_eq!(v["status"], "invalid");
    push("a method's validation view", v.to_string());

    let mut other = recipe(7, NAME, "mean", &["C1", "C2"]);
    other["capabilityId"] = json!("stats.describe");
    let mut stale = recipe(7, NAME, "mean", &["C1", "C2"]);
    stale["capabilityVersion"] = json!(9);
    let mut not_an_id = recipe(7, NAME, "mean", &["C1", "C2"]);
    not_an_id["capabilityId"] = json!("compute.anything");
    for (cmd, body) in [
        ("stats_prep_make", json!({ "slot": 99, "recipe": mean })),
        ("stats_prep_make", json!({ "slot": slot, "recipe": other })),
        ("stats_prep_make", json!({ "slot": slot, "recipe": stale })),
        (
            "stats_prep_make",
            json!({ "slot": slot, "recipe": not_an_id }),
        ),
        ("stats_prep_clear", json!({ "slot": 99 })),
        ("stats_prep_recipe", json!({ "slot": 99 })),
        ("stats_prep_csv", json!({ "slot": 99 })),
        (
            "stats_ttest_group_levels",
            json!({ "slot": slot, "column": "C8" }),
        ),
        (
            "stats_anova_group_levels",
            json!({ "slot": slot, "column": "C8" }),
        ),
        (
            "stats_ranks_group_levels",
            json!({ "slot": slot, "column": "C8" }),
        ),
        (
            "stats_desc_open",
            json!({ "path": folder.join(format!("{FILE}_missing.csv")).to_string_lossy() }),
        ),
    ] {
        errors.push(call(&w, cmd, body).expect_err(cmd));
    }

    // Replace with a name of its own, then clear: the name goes with it.
    let replaced = make(&w, &slot, &recipe(7, "another", "sum", &["C1", "C2"]));
    assert!(
        !replaced.to_string().contains(NAME),
        "the first name is gone"
    );
    let cleared = call(&w, "stats_prep_clear", json!({ "slot": slot })).unwrap();
    assert_eq!(cleared, o, "as opened");
    assert!(!cleared.to_string().contains(NAME));
    assert_eq!(
        call(&w, "stats_prep_csv", json!({ "slot": slot }))
            .map(|_| ())
            .unwrap_err()["code"],
        "validation"
    );
    assert_eq!(
        call(&w, "stats_desc_close", json!({ "slot": slot })).unwrap(),
        json!(true)
    );

    // Inactive: every command refused with the workspace's own code.
    call(&w, "workspace_deactivate", json!({})).expect("leaves");
    for (cmd, body) in [
        ("stats_prep_make", json!({ "slot": slot, "recipe": mean })),
        ("stats_prep_clear", json!({ "slot": slot })),
        ("stats_prep_recipe", json!({ "slot": slot })),
        ("stats_prep_csv", json!({ "slot": slot })),
    ] {
        errors.push(call(&w, cmd, body).expect_err(cmd));
    }
    for e in &errors {
        push("a refusal", e.to_string());
    }

    // Tracing: every span and event, from every thread.
    let traced = lines.lock().unwrap().clone();
    assert!(
        traced.iter().any(|l| l.contains("stats_prep: recipe held")),
        "the make's event was captured from spawn_blocking: {traced:?}"
    );
    assert!(
        traced.iter().any(|l| l.contains("stats_prep: refused")),
        "the refusals were traced"
    );
    assert!(
        traced
            .iter()
            .any(|l| l.contains("state=\"succeeded\"") && l.contains("run ended")),
        "the worker's events were captured"
    );
    for l in &traced {
        push("tracing", l.clone());
    }

    // Files: the app-data folder is unchanged, byte for byte.
    assert_eq!(
        before,
        file_hashes(&app_data),
        "a file in the app-data folder was written or created"
    );
    let mut files = Vec::new();
    files_under(&app_data, &mut files);
    for f in &files {
        let bytes = std::fs::read(f).unwrap();
        push(
            &format!("file {}", f.strip_prefix(&app_data).unwrap().display()),
            String::from_utf8_lossy(&bytes).into_owned(),
        );
    }
    let mut written = Vec::new();
    files_under(&folder, &mut written);
    assert_eq!(written.len(), 1, "only the test's own file: {written:?}");

    // The matrix.
    let mut grid: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut leaks = Vec::new();
    for (surface, text) in &surfaces {
        let row = grid
            .entry(surface.clone())
            .or_insert_with(|| vec!["-"; SENTINELS.len()]);
        for (i, s) in SENTINELS.iter().enumerate() {
            if text.contains(s) {
                if allowed(surface, s) {
                    row[i] = "allowed";
                } else {
                    row[i] = "LEAK";
                    let at = text.find(s).unwrap_or(0);
                    let from = text.floor_char_boundary(at.saturating_sub(60));
                    let to = text.floor_char_boundary((at + s.len() + 20).min(text.len()));
                    leaks.push(format!("{s} in {surface}: …{}…", &text[from..to]));
                }
            }
        }
    }
    println!(
        "{:<44} {}",
        "surface",
        SENTINELS.map(|s| format!("{s:<18}")).join(" ")
    );
    for (surface, row) in &grid {
        println!(
            "{surface:<44} {}",
            row.iter()
                .map(|c| format!("{c:<18}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    println!(
        "surfaces: {}; refusals: {}; tracing lines: {}; files: {}",
        surfaces.len(),
        errors.len(),
        traced.len(),
        files.len()
    );
    assert!(
        leaks.is_empty(),
        "sentinels where they may not be:\n{}",
        leaks.join("\n")
    );

    // Not vacuous: each allowed place holds it, and every forbidden surface was
    // collected.
    let at =
        |surface: &str, s: &str| grid[surface][SENTINELS.iter().position(|x| *x == s).unwrap()];
    assert_eq!(at("DatasetView", NAME), "allowed");
    assert_eq!(at("DatasetView", HDR), "allowed");
    assert_eq!(at("DatasetView", FILE), "allowed");
    assert_eq!(at("recipe in its request", NAME), "allowed");
    assert_eq!(at("recipe in its response", NAME), "allowed");
    assert_eq!(at("prepared CSV", NAME), "allowed");
    assert_eq!(at("prepared CSV", SCORE_VALUE), "allowed");
    for surface in [
        "spec",
        "a method's result (without its manifest)",
        "a method's result CSV",
        "manifest (in the result view)",
        "manifest (exported bytes)",
        "tracing",
        "a refusal",
        "a refused make's report",
    ] {
        assert!(grid.contains_key(surface), "{surface} was collected");
    }
}

/// The negative control: a score constant at the sentinel, whose mean the
/// descriptive summary's result shows, so `contains` can find it there. It runs
/// the core directly, with no command and no tracing of its own: the test above
/// owns the process's subscriber.
#[test]
fn the_summary_holds_the_value_when_a_score_is_constant_at_it() {
    use gaply_core::contracts::CapabilityId;
    use gaply_core::stats_desc::{self, Alias, ColumnRequest, DescribeParams, TreatAs};
    use gaply_core::stats_l1::dataset::{
        build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef,
    };
    use gaply_core::stats_prep::{
        self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic,
    };

    let rows: Vec<Vec<String>> = (0..6)
        .map(|_| vec![ITEM_VALUE.to_string(), "85.25".to_string()])
        .collect();
    let d = build_dataset(
        RawTable::from_rows(vec![HDR.into(), "q2".into()], rows),
        DataFingerprint::of_bytes(b"negative control", "test", TableRef::FirstTable),
    )
    .expect("dataset");
    let alias = |n: u32| Alias(ColumnAlias(n));
    let recipe = Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: 2,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: alias(3),
            name: NAME.into(),
            items: vec![alias(1), alias(2)],
            reversed: Vec::new(),
            scale: None,
            statistic: Some(Statistic::Mean),
            rule: Some(MissingRule::EveryItem),
            missing_codes: Vec::new(),
        })],
    };
    let prepared = stats_prep::build(&d, &recipe, 1_000).expect("builds");
    let result = stats_desc::describe(
        &prepared.dataset,
        &DescribeParams {
            columns: vec![ColumnRequest {
                alias: alias(3),
                treat_as: TreatAs::Numeric,
            }],
            missing_codes: Vec::new(),
        },
    )
    .expect("the summary computes");
    let json = String::from_utf8(result.canonical_bytes()).unwrap();
    assert!(
        json.contains(SCORE_VALUE),
        "73.53125 is every row's score, so the summary's mean must show it: {json}"
    );
    assert!(!json.contains(NAME), "and still no name: {json}");
    assert!(json.contains("\"header\":\"C3\""), "{json}");
}
