//! **Scale scores through IPC (Research OS, Phase 8M-3, decisions 425 to 427),
//! with the webview's argument names and the JSON it reads.** The four commands
//! and their refusals; all eight methods on a prepared slot, each spec copying
//! the view's inputRef and preparation; every way a spec can be for another
//! table; a score refused as a group; and making, replacing and clearing refused
//! while a run is going, as a second run is. Every file is synthetic and in a
//! folder the test creates and removes.

mod desc_support;
mod prep_support;

use serde_json::{json, Value};

use desc_support::{call, refusal};
use prep_support::*;

const NAME: &str = "wellbeing";

fn code(e: &Value) -> String {
    refusal(e)["code"].as_str().expect("a code").to_string()
}

fn prepared(tag: &str) -> (World, Value, Value) {
    let w = world(tag, TABLE, None);
    let o = open(&w);
    let view = make(&w.w, &o["slot"], &mean_recipe(NAME));
    (w, o, view)
}

/* ---- the commands ---------------------------------------------------------------- */

#[test]
fn make_read_export_and_clear_run_through_ipc() {
    let w = world("flow", TABLE, None);
    let o = open(&w);
    let slot = &o["slot"];
    assert_eq!(
        call(&w.w, "stats_prep_recipe", json!({ "slot": slot })).unwrap(),
        Value::Null,
        "no recipe yet"
    );

    let recipe = mean_recipe(NAME);
    let made = call(
        &w.w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": recipe }),
    )
    .unwrap();
    assert_eq!(made["status"], "valid", "{made}");
    assert_eq!(made["counts"][0]["column"], SCORE);
    assert_eq!(made["counts"][0]["scoredFromEveryItem"], 12);
    assert_eq!(made["estimatedResources"]["columns"], 8);
    let view = &made["dataset"];
    assert_eq!(view["slot"], *slot);
    assert_eq!(view["displayName"], o["displayName"]);
    assert_eq!(view["profile"]["columns"][7]["scoreName"], NAME);
    assert_eq!(view["profile"]["columns"][7]["prepared"], true);
    assert_ne!(view["inputRef"], o["inputRef"]);
    assert_eq!(view["inputRef"]["columns"], 8);
    assert_eq!(
        view["preparation"]["sourceHash"],
        o["inputRef"]["contentHash"]
    );
    assert_eq!(view["preparation"]["sourceColumns"], 7);

    let read = call(&w.w, "stats_prep_recipe", json!({ "slot": slot })).unwrap();
    assert_eq!(read["recipe"], recipe, "what the user sent");
    assert_eq!(read["counts"], made["counts"]);

    let csv = call(&w.w, "stats_prep_csv", json!({ "slot": slot })).unwrap();
    let bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        text.starts_with(&format!(
            "\"q1\",\"q2\",\"q3\",\"q4\",\"arm\",\"site\",\"age\",\"{NAME}\"\r\n"
        )),
        "{text}"
    );
    assert_eq!(text.lines().count(), 13);
    assert_eq!(csv["sha256"].as_str().unwrap().len(), 64);

    let cleared = call(&w.w, "stats_prep_clear", json!({ "slot": slot })).unwrap();
    assert_eq!(
        cleared, o,
        "the slot back on the file's table, byte for byte"
    );
    assert_eq!(
        call(&w.w, "stats_prep_recipe", json!({ "slot": slot })).unwrap(),
        Value::Null
    );
    assert_eq!(
        code(&call(&w.w, "stats_prep_csv", json!({ "slot": slot })).unwrap_err()),
        "run.unknown_input"
    );
}

#[test]
fn a_refused_recipe_is_a_report_with_no_dataset() {
    let w = world("report", TABLE, None);
    let o = open(&w);
    let mut bad = mean_recipe(NAME);
    bad["steps"][0]["statistic"] = Value::Null;
    let v = call(
        &w.w,
        "stats_prep_make",
        json!({ "slot": o["slot"], "recipe": bad }),
    )
    .unwrap();
    assert_eq!(v["status"], "needs_user_input", "{v}");
    assert_eq!(
        v["missingInformation"][0]["code"],
        "stats.prep.statistic_not_chosen"
    );
    assert_eq!(v["dataset"], Value::Null);
    assert_eq!(
        call(&w.w, "stats_prep_recipe", json!({ "slot": o["slot"] })).unwrap(),
        Value::Null,
        "nothing was held"
    );

    let mut sum_fewer = mean_recipe(NAME);
    sum_fewer["steps"][0]["statistic"] = json!("sum");
    sum_fewer["steps"][0]["rule"] = json!({ "at_least": 2 });
    let v = call(
        &w.w,
        "stats_prep_make",
        json!({ "slot": o["slot"], "recipe": sum_fewer }),
    )
    .unwrap();
    assert_eq!(v["status"], "valid_with_warnings", "{v}");
    assert_eq!(v["warnings"][0]["code"], "stats.prep.sum_over_fewer_items");
    assert!(v["dataset"].is_object(), "a warning makes it all the same");
}

#[test]
fn every_command_checks_the_workspace_the_capability_and_the_slot() {
    let w = world("refusals", TABLE, None);
    let o = open(&w);
    let slot = &o["slot"];
    let recipe = mean_recipe(NAME);

    // A slot nobody holds.
    for (cmd, body) in [
        ("stats_prep_make", json!({ "slot": 99, "recipe": recipe })),
        ("stats_prep_clear", json!({ "slot": 99 })),
        ("stats_prep_recipe", json!({ "slot": 99 })),
        ("stats_prep_csv", json!({ "slot": 99 })),
    ] {
        assert_eq!(
            code(&call(&w.w, cmd, body).unwrap_err()),
            "run.unknown_input",
            "{cmd}"
        );
    }

    // The registry's own refusals, returned as they are (decision 74).
    let mut other = recipe.clone();
    other["capabilityId"] = json!("stats.describe");
    assert_eq!(
        code(
            &call(
                &w.w,
                "stats_prep_make",
                json!({ "slot": slot, "recipe": other })
            )
            .unwrap_err()
        ),
        "capability.unknown_capability"
    );
    let mut stale = recipe.clone();
    stale["capabilityVersion"] = json!(9);
    assert_eq!(
        code(
            &call(
                &w.w,
                "stats_prep_make",
                json!({ "slot": slot, "recipe": stale })
            )
            .unwrap_err()
        ),
        "capability.version_mismatch"
    );
    // An id of a workspace this build does not have is no capability id at all.
    let mut elsewhere = recipe.clone();
    elsewhere["capabilityId"] = json!("compute.anything");
    assert!(call(
        &w.w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": elsewhere })
    )
    .is_err());
    // An unknown field reaches no command.
    let mut extra = recipe.clone();
    extra["steps"][0]["note"] = json!("a field no recipe has");
    assert!(call(
        &w.w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": extra })
    )
    .is_err());

    // Inactive: the workspace's own code, for every command.
    call(&w.w, "workspace_deactivate", json!({})).expect("leaves");
    for (cmd, body) in [
        ("stats_prep_make", json!({ "slot": slot, "recipe": recipe })),
        ("stats_prep_clear", json!({ "slot": slot })),
        ("stats_prep_recipe", json!({ "slot": slot })),
        ("stats_prep_csv", json!({ "slot": slot })),
    ] {
        assert_eq!(
            code(&call(&w.w, cmd, body).unwrap_err()),
            "workspace.inactive",
            "{cmd}"
        );
    }
}

/* ---- all eight methods on a prepared slot ------------------------------------------ */

#[test]
fn all_eight_methods_run_on_a_score_through_ipc_naming_it_by_its_alias_alone() {
    let (w, o, view) = prepared("eight");
    let methods = eight(&view, SCORE);
    assert_eq!(methods.len(), 8);
    for m in &methods {
        let report = call(
            &w.w,
            &format!("{}_validate", m.prefix),
            json!({ "slot": o["slot"], "spec": m.spec }),
        )
        .unwrap_or_else(|e| panic!("{}: {e}", m.prefix));
        assert!(
            ["valid", "valid_with_warnings"].contains(&report["status"].as_str().unwrap()),
            "{}: {report}",
            m.prefix
        );
        let run = start(&w.w, &o["slot"], m).unwrap_or_else(|e| panic!("{}: {e}", m.prefix));
        let state = until_terminal(&w.w, m.prefix, run);
        assert_eq!(state["state"], "succeeded", "{}: {state}", m.prefix);
        let result = call(
            &w.w,
            &format!("{}_result", m.prefix),
            json!({ "runId": run }),
        )
        .unwrap_or_else(|e| panic!("{}: {e}", m.prefix));

        assert_eq!(
            result["manifest"]["preparation"], view["preparation"],
            "{}: the manifest carries the preparation",
            m.prefix
        );
        assert_eq!(
            result["manifest"]["inputs"][0], view["inputRef"]["contentHash"],
            "{}: the prepared input",
            m.prefix
        );
        let text = result.to_string();
        // The t-tests and the ANOVA print no column at all (ADR-011, premise 3).
        if ["stats_desc", "stats_corr", "stats_alpha"].contains(&m.prefix) {
            assert!(
                text.contains(&format!("\"{SCORE}\"")),
                "{}: by its alias: {text}",
                m.prefix
            );
        }
        assert!(
            !text.contains(NAME),
            "{}: never by its typed name",
            m.prefix
        );
        let csv = call(
            &w.w,
            &format!("{}_export", m.prefix),
            json!({ "runId": run, "format": "csv" }),
        )
        .unwrap_or_else(|e| panic!("{}: {e}", m.prefix));
        let bytes: Vec<u8> = serde_json::from_value(csv["bytes"].clone()).unwrap();
        assert!(
            !String::from_utf8(bytes).unwrap().contains(NAME),
            "{}: its CSV holds no typed name",
            m.prefix
        );
    }
}

/* ---- a spec for another table ------------------------------------------------------- */

/// Each method's refusal, from validate and from a started run.
fn assert_refused(w: &World, slot: &Value, m: &Method, why: &str) {
    let word = m
        .prefix
        .trim_start_matches("stats_")
        .replace("paired", "onesample");
    let report = call(
        &w.w,
        &format!("{}_validate", m.prefix),
        json!({ "slot": slot, "spec": m.spec }),
    )
    .unwrap_or_else(|e| panic!("{why} {}: {e}", m.prefix));
    assert!(
        error_codes(&report).contains(&format!("stats.{word}.preparation_mismatch")),
        "{why} {}: {report}",
        m.prefix
    );
    let run = start(&w.w, slot, m).unwrap_or_else(|e| panic!("{why} {}: {e}", m.prefix));
    let state = until_terminal(&w.w, m.prefix, run);
    assert_eq!(state["state"], "refused", "{why} {}: {state}", m.prefix);
}

#[test]
fn a_spec_with_a_forged_preparation_is_refused_by_every_method() {
    let (w, o, view) = prepared("forged");
    let forgeries: [(&str, Value); 3] = [
        ("recipeHash", json!("ab".repeat(32))),
        ("sourceHash", json!("cd".repeat(32))),
        ("sourceColumns", json!(6)),
    ];
    for (field, value) in forgeries {
        for mut m in eight(&view, SCORE) {
            m.spec["preparation"][field] = value.clone();
            assert_refused(&w, &o["slot"], &m, field);
        }
    }
    // None at all, on a prepared slot.
    for mut m in eight(&view, SCORE) {
        m.spec.as_object_mut().unwrap().remove("preparation");
        assert_refused(&w, &o["slot"], &m, "absent");
    }
}

#[test]
fn two_recipes_of_one_width_are_refused_for_each_other() {
    let (w, o, first) = prepared("two-recipes");
    let on_first = eight(&first, SCORE);
    let second = make(&w.w, &o["slot"], &sum_recipe("total"));
    assert_eq!(first["inputRef"]["columns"], second["inputRef"]["columns"]);
    assert_ne!(first["inputRef"], second["inputRef"]);
    for m in &on_first {
        assert_refused(&w, &o["slot"], m, "the first recipe's spec");
    }
    let on_second = eight(&second, SCORE);
    make(&w.w, &o["slot"], &mean_recipe(NAME));
    for m in &on_second {
        assert_refused(&w, &o["slot"], m, "the second recipe's spec");
    }
}

#[test]
fn a_spec_made_on_the_original_is_refused_on_the_prepared_slot_and_the_reverse() {
    let w = world("carried", TABLE, None);
    let o = open(&w);
    // On the file's table, on a file's column, so only the table differs.
    let on_original = eight(&o, "C3");
    for m in &on_original {
        let report = call(
            &w.w,
            &format!("{}_validate", m.prefix),
            json!({ "slot": o["slot"], "spec": m.spec }),
        )
        .unwrap();
        assert!(error_codes(&report).is_empty(), "{}: {report}", m.prefix);
    }
    let view = make(&w.w, &o["slot"], &mean_recipe(NAME));
    for m in &on_original {
        assert_refused(&w, &o["slot"], m, "made on the original");
    }
    let on_prepared = eight(&view, "C3");
    call(&w.w, "stats_prep_clear", json!({ "slot": o["slot"] })).unwrap();
    for m in &on_prepared {
        assert_refused(&w, &o["slot"], m, "made on the prepared table");
    }
    // And once cleared, the original's spec runs again.
    for m in &on_original {
        let run = start(&w.w, &o["slot"], m).unwrap();
        assert_eq!(
            until_terminal(&w.w, m.prefix, run)["state"],
            "succeeded",
            "{}",
            m.prefix
        );
    }
}

/* ---- a score is never a group --------------------------------------------------------- */

#[test]
fn a_score_as_a_group_is_refused_by_the_ttest_and_the_anova_through_ipc() {
    let (w, o, view) = prepared("group");
    for (prefix, word, position) in [("stats_ttest", "ttest", 1), ("stats_anova", "anova", 4)] {
        let mut m = eight(&view, "C3").remove(position);
        assert_eq!(m.prefix, prefix);
        m.spec["parameters"]["group"] = json!(SCORE);
        let report = call(
            &w.w,
            &format!("{prefix}_validate"),
            json!({ "slot": o["slot"], "spec": m.spec }),
        )
        .unwrap();
        assert!(
            error_codes(&report).contains(&format!("stats.{word}.score_as_group")),
            "{prefix}: {report}"
        );
    }
}

/* ---- while a run is going ---------------------------------------------------------------- */

#[test]
fn making_replacing_and_clearing_are_refused_while_a_run_is_going() {
    let gate = Gate::new();
    let w = world("busy", TABLE, Some(gate.runs()));
    let o = open(&w);
    let slot = &o["slot"];

    // A make while a run goes on the file's table.
    let on_file = eight(&o, "C3").remove(0);
    let run = start(&w.w, slot, &on_file).unwrap();
    gate.wait_reached();
    assert_eq!(
        code(
            &call(
                &w.w,
                "stats_prep_make",
                json!({ "slot": slot, "recipe": mean_recipe(NAME) })
            )
            .unwrap_err()
        ),
        "run.run_busy"
    );
    assert_eq!(
        call(&w.w, "stats_prep_recipe", json!({ "slot": slot })).unwrap(),
        Value::Null,
        "nothing was held"
    );
    gate.release();
    assert_eq!(
        until_terminal(&w.w, on_file.prefix, run)["state"],
        "succeeded"
    );

    // Now with a recipe held, and a second gate.
    let gate = Gate::new();
    let w = world("busy-2", TABLE, Some(gate.runs()));
    let o = open(&w);
    let slot = &o["slot"];
    let view = make(&w.w, slot, &mean_recipe(NAME));
    let m = eight(&view, SCORE).remove(0);
    let run = start(&w.w, slot, &m).unwrap();
    gate.wait_reached();
    assert_eq!(
        code(
            &call(
                &w.w,
                "stats_prep_make",
                json!({ "slot": slot, "recipe": sum_recipe("total") })
            )
            .unwrap_err()
        ),
        "run.run_busy",
        "a replace"
    );
    assert_eq!(
        code(&call(&w.w, "stats_prep_clear", json!({ "slot": slot })).unwrap_err()),
        "run.run_busy"
    );
    // The reads are not refused.
    let read = call(&w.w, "stats_prep_recipe", json!({ "slot": slot })).unwrap();
    assert_eq!(
        read["recipe"]["steps"][0]["name"], NAME,
        "it is still the first"
    );
    call(&w.w, "stats_prep_csv", json!({ "slot": slot })).expect("the CSV reads");

    gate.release();
    assert_eq!(until_terminal(&w.w, m.prefix, run)["state"], "succeeded");
    let cleared =
        call(&w.w, "stats_prep_clear", json!({ "slot": slot })).expect("the run has ended");
    assert_eq!(cleared["inputRef"], o["inputRef"]);
}
