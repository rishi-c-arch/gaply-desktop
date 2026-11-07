//! **Scale scores' wire contract for the screen, from the Rust side (Research
//! OS, Phase 8T-1b, decision 499), in the pattern of `stats_alpha_wire.rs`.**
//!
//! * `prepCodes.json`, the code list `prepWording.ts` is keyed by, is exactly
//!   `PrepCode::ALL`, the two run codes the prep commands give
//!   (`run.run_busy`, `run.unknown_input`), `WorkspaceCode::ALL` and
//!   `CapabilityCode::ALL`, sorted: 59 codes.
//! * Three views, as the webview reads them through IPC on a synthetic file:
//!   a made view, a refused make and a recipe. Each is the exact bytes of its
//!   fixture in `src/screens/statistics/fixtures/` (pretty JSON, two-space
//!   indent, a final newline), which `prepTypes.vitest.ts` deep-compares with a
//!   copy typed `satisfies` its view. Through IPC a view is a
//!   `serde_json::Value`, whose keys come out sorted; that order is the
//!   fixture's.
//!
//! Written with `GAPLY_WRITE_PREP_WIRE` set, read, and then pinned.

mod desc_support;
mod prep_support;

use std::path::PathBuf;

use serde_json::{json, Value};

use desc_support::call;
use gaply_core::contracts::capability::CapabilityCode;
use gaply_core::contracts::codes::{Closed, RunCode};
use gaply_core::contracts::workspace::WorkspaceCode;
use gaply_core::stats_prep::PrepCode;
use prep_support::{open, world, TABLE};

fn screens() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/screens/statistics")
}

fn writing() -> bool {
    std::env::var_os("GAPLY_WRITE_PREP_WIRE").is_some()
}

fn assert_bytes(name: &str, view: &Value) {
    let path = screens().join("fixtures").join(name);
    let got = serde_json::to_string_pretty(view).unwrap() + "\n";
    if writing() {
        std::fs::write(&path, &got).unwrap();
    }
    let want = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{name}: {e}; the view is:\n{got}"));
    assert_eq!(got, want, "{name}: the view and its fixture differ");
}

/* ---- the code list --------------------------------------------------------------- */

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn the_prep_code_list_is_exactly_the_closed_sets_the_prep_commands_give() {
    let path = screens().join("prepCodes.json");
    let mut rust: Vec<String> = PrepCode::ALL.iter().map(|c| c.wire().to_string()).collect();
    assert_eq!(rust.len(), 46, "scale scores' 46 codes");
    rust.extend(
        [RunCode::RunBusy, RunCode::UnknownInput]
            .iter()
            .map(|c| c.wire().to_string()),
    );
    rust.extend(WorkspaceCode::ALL.iter().map(|c| c.wire().to_string()));
    rust.extend(CapabilityCode::ALL.iter().map(|c| c.wire().to_string()));
    let rust = sorted(rust);
    assert_eq!(rust.len(), 59, "59 codes in all");
    if writing() {
        std::fs::write(&path, serde_json::to_string_pretty(&rust).unwrap() + "\n").unwrap();
    }
    let listed: Vec<String> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("prepCodes.json")).unwrap();
    assert_eq!(
        listed, rust,
        "prepCodes.json and the Rust closed sets differ, or it is not sorted"
    );
}

/* ---- the three views, through IPC -------------------------------------------------- */

/// A mean of three items, one reversed on 1 to 5, at least two items needed, one
/// missing code: every field of a step filled.
fn made_recipe() -> Value {
    json!({
        "schemaVersion": 1,
        "capabilityId": "stats.prepare.scale_score",
        "capabilityVersion": 1,
        "engineVersion": 1,
        "sourceColumns": 7,
        "steps": [{
            "kind": "scale_score",
            "alias": "C8",
            "name": "wellbeing",
            "items": ["C1", "C2", "C3"],
            "reversed": ["C2"],
            "scale": { "min": 1.0, "max": 5.0 },
            "statistic": "mean",
            "rule": { "at_least": 2 },
            "missingCodes": [9.0],
        }],
    })
}

/// Two scores: a sum under "at least 1" (a note), then one with one item, a
/// reversed column that is not an item, no ends and no statistic: an error with
/// a count, one with a column, one with a step only, and a question.
fn refused_recipe() -> Value {
    json!({
        "schemaVersion": 1,
        "capabilityId": "stats.prepare.scale_score",
        "capabilityVersion": 1,
        "engineVersion": 1,
        "sourceColumns": 7,
        "steps": [
            {
                "kind": "scale_score",
                "alias": "C8",
                "name": "first",
                "items": ["C1", "C2", "C3"],
                "reversed": [],
                "scale": null,
                "statistic": "sum",
                "rule": { "at_least": 1 },
                "missingCodes": [],
            },
            {
                "kind": "scale_score",
                "alias": "C9",
                "name": "second",
                "items": ["C4"],
                "reversed": ["C2"],
                "scale": null,
                "statistic": null,
                "rule": "every_item",
                "missingCodes": [],
            },
        ],
    })
}

#[test]
fn the_made_the_refused_and_the_recipe_views_are_their_fixtures_bytes() {
    let w = world("prep-screen-wire", TABLE, None);
    let opened = open(&w);
    let slot = opened["slot"].clone();

    let made = call(
        &w.w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": made_recipe() }),
    )
    .expect("the make");
    assert_eq!(made["status"], "valid", "{made}");
    assert!(made["dataset"].is_object(), "the slot now holds the score");
    assert_eq!(made["counts"].as_array().unwrap().len(), 1);
    assert_bytes("prepMadeView.json", &made);

    let held = call(&w.w, "stats_prep_recipe", json!({ "slot": slot })).expect("the recipe");
    assert_eq!(held["recipe"], made_recipe(), "the recipe as it was sent");
    assert_bytes("prepRecipeView.json", &held);

    let refused = call(
        &w.w,
        "stats_prep_make",
        json!({ "slot": slot, "recipe": refused_recipe() }),
    )
    .expect("a refused make is a view, not an error");
    assert_eq!(refused["status"], "invalid", "{refused}");
    assert!(refused["dataset"].is_null(), "no table was held");
    for key in ["errors", "warnings", "missingInformation"] {
        assert!(
            !refused[key].as_array().unwrap().is_empty(),
            "the refused view fills {key}: {refused}"
        );
    }
    assert_bytes("prepMadeViewRefused.json", &refused);

    // A refused make changes nothing: the slot still holds the first recipe.
    let still = call(&w.w, "stats_prep_recipe", json!({ "slot": slot })).expect("the recipe");
    assert_eq!(still, held);
}
