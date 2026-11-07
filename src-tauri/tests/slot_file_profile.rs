//! **A make profiles only its new columns (Research OS, Phase 8T-1b, decision
//! 498).** A slot keeps the profile of its file's own columns from the moment
//! it is held: after an open, after an insert, and through a make and a clear,
//! which share it as they share the file's bytes. `view_of` reads that profile
//! and profiles only the columns a recipe added, so a make no longer profiles
//! the whole table again.
//!
//! Three things are pinned:
//! * the slot holds its file profile at each of those four points, the same
//!   allocation through a make and a clear, and equal to the core's profile of
//!   the file's table;
//! * `view_of`'s body in `stats_desc_commands.rs` calls no
//!   `stats_desc::profile(` (a source scan, so a whole-table profile cannot
//!   come back unseen);
//! * the bytes oracle: the view's profile, its overlay (`prepared`,
//!   `scoreName`) taken off, is exactly the core's profile of the whole held
//!   table, for one score, two scores in one make, a make after a make, a
//!   clear, and a file with a categorical column and a suspect code. This
//!   passes before the change by design; its red is the mutation that puts the
//!   new columns first.
//!
//! Synthetic data, in memory or in a folder the test creates and removes.

mod desc_support;

use serde_json::Value;

use app_lib::runs::{DatasetSlots, Runs, SourceFile};
use app_lib::stats_desc_commands as desc;
use app_lib::stats_prep_commands as prep;
use desc_support::TempDir;
use gaply_core::contracts::{CapabilityId, ContentHash};
use gaply_core::research_project::DataFormat;
use gaply_core::stats_desc::{self, Alias, DatasetProfile};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};

const CAP: u64 = 1_000_000;

fn alias(n: usize) -> Alias {
    Alias(ColumnAlias(n as u32))
}

/// `columns` numeric columns of 1 to 5.
fn table(columns: usize, rows: usize, tag: &[u8]) -> Dataset {
    let headers = (1..=columns).map(|c| format!("q{c}")).collect();
    let cells = (0..rows)
        .map(|r| {
            (0..columns)
                .map(|c| format!("{}", 1 + (r * 7 + c * 3) % 5))
                .collect()
        })
        .collect();
    build_dataset(
        RawTable::from_rows(headers, cells),
        DataFingerprint::of_bytes(tag, "test", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// A categorical column first, three items of 1 to 5, and a column of 1 to 5
/// holding -99 in two rows, which Level 1's rule flags as a possible code.
fn mixed(rows: usize) -> Dataset {
    let headers = ["arm", "q1", "q2", "q3", "age"]
        .iter()
        .map(|h| h.to_string())
        .collect();
    let cells = (0..rows)
        .map(|r| {
            vec![
                ["a", "b", "c"][r % 3].to_string(),
                format!("{}", 1 + (r * 7) % 5),
                format!("{}", 1 + (r * 3 + 1) % 5),
                format!("{}", 1 + (r * 2 + 4) % 5),
                if r % 5 == 2 {
                    "-99".to_string()
                } else {
                    format!("{}", 1 + r % 5)
                },
            ]
        })
        .collect();
    build_dataset(
        RawTable::from_rows(headers, cells),
        DataFingerprint::of_bytes(b"mixed", "test", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn step(alias_n: usize, name: &str, items: &[usize], statistic: Statistic) -> ScaleScoreStep {
    ScaleScoreStep {
        alias: alias(alias_n),
        name: name.into(),
        items: items.iter().copied().map(alias).collect(),
        reversed: Vec::new(),
        scale: None,
        statistic: Some(statistic),
        rule: Some(MissingRule::EveryItem),
        missing_codes: Vec::new(),
    }
}

fn recipe(columns: usize, steps: Vec<ScaleScoreStep>) -> Recipe {
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: columns as u32,
        steps: steps.into_iter().map(RecipeStep::ScaleScore).collect(),
    }
}

fn made(runs: &Runs, slots: &DatasetSlots, slot: u64, r: &Recipe) -> desc::DatasetView {
    prep::make(runs, slots, slot, r, CAP)
        .map_err(|_| "make refused")
        .unwrap()
        .dataset
        .expect("the prepared view")
}

fn cleared(runs: &Runs, slots: &DatasetSlots, slot: u64) -> desc::DatasetView {
    prep::clear(runs, slots, slot)
        .map_err(|_| "clear refused")
        .unwrap()
}

fn json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

/* ---- the slot holds its file profile ------------------------------------------ */

/// The slot's file profile: present, and the core's profile of `file`.
fn holds(slots: &DatasetSlots, slot: u64, file: &Dataset, when: &str) -> *const DatasetProfile {
    let held = slots.get(slot).unwrap();
    let p = held
        .file_profile()
        .unwrap_or_else(|| panic!("{when}: the slot holds no file profile"));
    assert_eq!(
        json(p),
        json(&stats_desc::profile(file)),
        "{when}: the held profile is the file's"
    );
    p as *const DatasetProfile
}

#[test]
fn a_slot_holds_its_file_profile_after_an_open_an_insert_a_make_and_a_clear() {
    // After an open, through the ingest.
    let dir = TempDir::new("file-profile");
    let path = dir.write("survey.csv", b"q1,q2,q3\n1,2,3\n2,3,4\n3,4,5\n4,5,1\n");
    let slots = DatasetSlots::default();
    let opened = desc::open(&slots, &path)
        .map_err(|_| "open refused")
        .unwrap();
    let held = slots.get(opened.slot).unwrap();
    let file = held.dataset().clone();
    drop(held);
    holds(&slots, opened.slot, &file, "after an open");

    // After an insert, with and without the file's bytes.
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let d = table(3, 10, b"a");
    let bare = slots.insert("synthetic.xlsx".into(), d.clone());
    holds(&slots, bare, &d, "after an insert");
    let hash = ContentHash::from_hex(d.schema.fingerprint.sha256_hex()).unwrap();
    let a = slots.insert_with_source(
        "synthetic.csv".into(),
        d.clone(),
        Some(SourceFile::new(b"bytes".to_vec(), hash, DataFormat::Csv)),
    );
    let first = holds(&slots, a, &d, "after an insert with its bytes");

    // A make and a clear share it, as they share the bytes.
    made(
        &runs,
        &slots,
        a,
        &recipe(3, vec![step(4, "a mean", &[1, 2], Statistic::Mean)]),
    );
    let after_make = holds(&slots, a, &d, "after a make");
    assert!(
        std::ptr::eq(first, after_make),
        "a make shares the held profile; it does not profile the file again"
    );
    cleared(&runs, &slots, a);
    let after_clear = holds(&slots, a, &d, "after a clear");
    assert!(
        std::ptr::eq(first, after_clear),
        "a clear shares the held profile"
    );
}

/* ---- the source: view_of profiles no file column ------------------------------- */

#[test]
fn view_of_never_profiles_the_whole_table() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stats_desc_commands.rs"),
    )
    .expect("stats_desc_commands.rs");
    let start = src
        .find("pub fn view_of(")
        .expect("view_of is in stats_desc_commands.rs");
    let len = src[start..]
        .find("\n}\n")
        .expect("view_of's body ends at a closing brace at column 0");
    let body = &src[start..start + len];
    assert!(
        body.lines().count() > 10,
        "view_of's body was read: the scan is not inert"
    );
    assert!(
        body.contains("DatasetView {"),
        "the body read is view_of's own"
    );
    assert!(
        !body.contains("stats_desc::profile("),
        "view_of profiles the whole table again; it must read the slot's file profile \
         and profile only the columns a recipe added"
    );
}

/* ---- the bytes oracle ----------------------------------------------------------- */

/// The view's profile with its overlay taken off is the core's profile of the
/// whole held table: the comparison at `stats_prep_slot.rs:458-465`, over the
/// whole profile, rows included.
fn is_the_full_profile(slots: &DatasetSlots, view: &desc::DatasetView, case: &str) {
    let mut got = json(&view.profile);
    for c in got["columns"].as_array_mut().unwrap() {
        let c = c.as_object_mut().unwrap();
        c.remove("prepared");
        c.remove("scoreName");
    }
    let held = slots.get(view.slot).unwrap();
    let want = json(&stats_desc::profile(held.dataset()));
    assert_eq!(got, want, "{case}: the view's profile is a full profile's");
    assert_eq!(
        json(&desc::view_of(&held)),
        json(view),
        "{case}: view_of gives the same bytes again"
    );
}

#[test]
fn the_views_profile_is_a_full_profiles_bytes_after_every_make_and_clear() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let a = slots.insert("synthetic.csv".into(), table(4, 12, b"oracle"));

    let one = made(
        &runs,
        &slots,
        a,
        &recipe(4, vec![step(5, "one", &[1, 2, 3], Statistic::Mean)]),
    );
    assert_eq!(one.profile.columns.len(), 5);
    is_the_full_profile(&slots, &one, "one score");

    let two = made(
        &runs,
        &slots,
        a,
        &recipe(
            4,
            vec![
                step(5, "first", &[1, 2], Statistic::Sum),
                step(6, "second", &[2, 3, 4], Statistic::Mean),
            ],
        ),
    );
    assert_eq!(two.profile.columns.len(), 6);
    assert_eq!(two.profile.columns[4].score_name.as_deref(), Some("first"));
    assert_eq!(two.profile.columns[5].score_name.as_deref(), Some("second"));
    is_the_full_profile(&slots, &two, "two scores in one make, a make after a make");

    let again = made(
        &runs,
        &slots,
        a,
        &recipe(4, vec![step(5, "again", &[3, 4], Statistic::Sum)]),
    );
    assert_eq!(again.profile.columns.len(), 5);
    is_the_full_profile(&slots, &again, "a make after a make");

    let back = cleared(&runs, &slots, a);
    assert_eq!(back.profile.columns.len(), 4);
    is_the_full_profile(&slots, &back, "a clear");
}

#[test]
fn the_views_profile_is_a_full_profiles_bytes_with_a_categorical_column_and_a_suspect_code() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let d = mixed(15);
    let file = json(&stats_desc::profile(&d));
    assert_eq!(
        file["columns"][0]["kind"], "categorical",
        "the fixture has a categorical column"
    );
    assert!(
        file["columns"][0]["levels"]
            .as_array()
            .is_some_and(|l| l.len() == 3),
        "with its levels"
    );
    assert_eq!(
        file["columns"][4]["suspectMissingCodes"][0]["code"], -99,
        "the fixture has a suspect code"
    );
    let a = slots.insert("mixed.csv".into(), d);
    is_the_full_profile(
        &slots,
        &desc::view_of(&slots.get(a).unwrap()),
        "the file's own table",
    );

    let v = made(
        &runs,
        &slots,
        a,
        &recipe(5, vec![step(6, "items", &[2, 3, 4], Statistic::Mean)]),
    );
    is_the_full_profile(
        &slots,
        &v,
        "a score beside a categorical column and a suspect code",
    );
    let back = cleared(&runs, &slots, a);
    is_the_full_profile(&slots, &back, "cleared");
}
