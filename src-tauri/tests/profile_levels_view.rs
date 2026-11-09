//! **Every view of a slot holds at most 50 levels a column (Research OS, Phase
//! 8W, decisions 566 and 568).** The cap is the core profile's, so the view an
//! open answers, `view_of`'s, a make's and a clear's agree, and the slot's held
//! file profile is the capped one: a label that sorts 51st in code-point order
//! is in none of them, while `distinct` still counts all 51.
//!
//! Synthetic data, in a folder the test creates and removes.

mod desc_support;

use serde_json::Value;

use app_lib::runs::{DatasetSlots, Runs};
use app_lib::stats_desc_commands as desc;
use app_lib::stats_prep_commands as prep;
use desc_support::TempDir;
use gaply_core::contracts::CapabilityId;
use gaply_core::stats_desc::{Alias, PROFILE_LEVELS_MAX};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};

/// The 51st label in code-point order, in more rows than any other.
const LAST: &str = "L50";

/// `g`, a text column of 51 levels "L00" to "L50", L50 in five rows; and two
/// numeric items.
fn csv() -> Vec<u8> {
    let mut s = String::from("g,q1,q2\n");
    for i in 0..51 {
        s.push_str(&format!("L{i:02},{},{}\n", 1 + i % 5, 1 + (i * 3) % 5));
    }
    for i in 0..4 {
        s.push_str(&format!("{LAST},{},{}\n", 1 + i, 2 + i % 3));
    }
    s.into_bytes()
}

fn score_of_items() -> Recipe {
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: 3,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: Alias(ColumnAlias(4)),
            name: "Average".into(),
            items: vec![Alias(ColumnAlias(2)), Alias(ColumnAlias(3))],
            reversed: Vec::new(),
            scale: None,
            statistic: Some(Statistic::Mean),
            rule: Some(MissingRule::EveryItem),
            missing_codes: Vec::new(),
        })],
    }
}

fn json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

/// The view's column g: at most 50 levels, the first 50 in code-point order,
/// distinct 51, and the 51st label nowhere in the view's bytes.
fn capped(view: &Value, when: &str) {
    let g = &view["profile"]["columns"][0];
    assert_eq!(g["distinct"], 51, "{when}: distinct counts every level");
    let levels = g["levels"].as_array().unwrap_or_else(|| panic!("{when}: a level list"));
    assert_eq!(levels.len(), PROFILE_LEVELS_MAX, "{when}: the list is capped");
    assert_eq!(levels[0]["label"], "L00", "{when}");
    assert_eq!(levels[49]["label"], "L49", "{when}");
    assert!(!view.to_string().contains(LAST), "{when}: the 51st label is in the view");
    for c in view["profile"]["columns"].as_array().unwrap() {
        let n = c["levels"].as_array().map_or(0, Vec::len);
        assert!(n <= PROFILE_LEVELS_MAX, "{when}: a column of {n} levels");
    }
}

#[test]
fn the_open_view_of_a_make_and_a_clear_each_hold_at_most_50_levels() {
    let dir = TempDir::new("profile-levels-view");
    let path = dir.write("levels.csv", &csv());
    let slots = DatasetSlots::default();
    let runs = Runs::default();

    let opened = desc::open(&slots, &path).map_err(|_| "open refused").unwrap();
    capped(&json(&opened), "the open's view");
    capped(&json(&desc::view_of(&slots.get(opened.slot).unwrap())), "view_of");

    let made = prep::make(&runs, &slots, opened.slot, &score_of_items(), 1_000_000)
        .map_err(|_| "make refused")
        .unwrap();
    let made_json = json(&made);
    capped(&made_json["dataset"], "a make's PrepMadeView");
    assert!(!made_json.to_string().contains(LAST), "the whole PrepMadeView");

    let cleared = prep::clear(&runs, &slots, opened.slot).map_err(|_| "clear refused").unwrap();
    capped(&json(&cleared), "stats_prep_clear's view");
}

#[test]
fn the_slot_holds_the_capped_profile() {
    let dir = TempDir::new("profile-levels-held");
    let path = dir.write("levels.csv", &csv());
    let slots = DatasetSlots::default();
    let opened = desc::open(&slots, &path).map_err(|_| "open refused").unwrap();
    let held = slots.get(opened.slot).unwrap();
    let p = held.file_profile().expect("the slot holds its file profile");
    let g = &p.columns[0];
    assert_eq!(g.distinct, 51);
    let levels = g.levels.as_ref().expect("a level list");
    assert_eq!(levels.len(), PROFILE_LEVELS_MAX, "the held profile is capped");
    assert!(levels.iter().all(|l| l.label != LAST));
}
