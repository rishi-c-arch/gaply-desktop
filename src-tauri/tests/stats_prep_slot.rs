//! **The prepared slot (Research OS, Phase 8M-3, decision 424), through the
//! commands' plain bodies, over slots the test holds.** One copy held, in the
//! same slot number and moved to the back; a refused make leaving everything as
//! it was; the original rebuilt rather than re-parsed; the original's
//! fingerprint kept; the bytes shared; each tier's cells cap; and what the view
//! says. Synthetic data, in memory or in a folder the test creates and removes.

use std::sync::{Arc, Weak};

use serde_json::{json, Value};

use app_lib::memory_tier::MemoryTier;
use app_lib::runs::{DatasetSlots, Runs, Slot, SourceFile};
use app_lib::stats_desc_commands as desc;
use app_lib::stats_prep_commands as prep;
use app_lib::supplementary::IngestLimits;
use gaply_core::contracts::codes::RunCode;
use gaply_core::contracts::{CapabilityId, ContentHash, ValidationStatus};
use gaply_core::research_project::DataFormat;
use gaply_core::sav_findings::{SavFinding, SavFindingCode, SavParam};
use gaply_core::stats_alpha::ScaleEnds;
use gaply_core::stats_desc::{self, Alias};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_prep::{self, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic};

const CAP: u64 = 1_000_000;
const BYTES: &[u8] = b"the file's own bytes";

fn alias(n: usize) -> Alias {
    Alias(ColumnAlias(n as u32))
}

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

fn step(columns: usize, name: &str, statistic: Option<Statistic>) -> ScaleScoreStep {
    ScaleScoreStep {
        alias: alias(columns + 1),
        name: name.into(),
        items: vec![alias(1), alias(2)],
        reversed: Vec::new(),
        scale: None,
        statistic,
        rule: Some(MissingRule::EveryItem),
        missing_codes: Vec::new(),
    }
}

fn recipe_of(columns: usize, step: ScaleScoreStep) -> Recipe {
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(stats_prep::CAPABILITY_ID).unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: columns as u32,
        steps: vec![RecipeStep::ScaleScore(step)],
    }
}

fn mean_of(columns: usize) -> Recipe {
    recipe_of(columns, step(columns, "a mean", Some(Statistic::Mean)))
}

fn sum_of(columns: usize) -> Recipe {
    recipe_of(columns, step(columns, "a sum", Some(Statistic::Sum)))
}

fn view_bytes(v: &desc::DatasetView) -> Value {
    serde_json::to_value(v).unwrap()
}

fn made(runs: &Runs, slots: &DatasetSlots, slot: u64, recipe: &Recipe) -> prep::PrepMadeView {
    prep::make(runs, slots, slot, recipe, CAP)
        .map_err(|_| "refused")
        .unwrap()
}

fn holding(slots: &DatasetSlots, slot: u64) -> (Arc<Slot>, Weak<Slot>) {
    let held = slots.get(slot).expect("the slot");
    let weak = Arc::downgrade(&held);
    (held, weak)
}

/// A slot with its bytes, as an open leaves it.
fn opened(slots: &DatasetSlots, d: Dataset) -> u64 {
    let hash = ContentHash::from_hex(d.schema.fingerprint.sha256_hex()).unwrap();
    slots.insert_with_source(
        "synthetic.csv".into(),
        d,
        Some(SourceFile::new(BYTES.to_vec(), hash, DataFormat::Csv)),
    )
}

/* ---- one copy, the same number, at the back ---------------------------------- */

#[test]
fn a_make_holds_one_copy_in_the_same_slot_moved_to_the_back() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let a = opened(&slots, table(3, 10, b"a"));
    let b = opened(&slots, table(3, 10, b"b"));
    assert_eq!(slots.order(), [a, b]);
    let (held, gone) = holding(&slots, a);
    drop(held);

    let v = made(&runs, &slots, a, &mean_of(3));
    let dataset = v.dataset.as_ref().expect("the prepared view");
    assert_eq!(dataset.slot, a, "the same slot number");
    assert_eq!(slots.order(), [b, a], "moved to the back");
    assert_eq!(slots.held(), 2, "no slot was added");
    assert_eq!(
        slots.cells_held(),
        40 + 30,
        "the prepared table's cells, once"
    );
    assert!(
        gone.upgrade().is_none(),
        "the file's table was dropped, not kept beside the prepared one"
    );
    let now = slots.get(a).unwrap();
    assert_eq!(now.dataset().schema.columns.len(), 4);
    assert_eq!(now.id(), a);
    assert_eq!(now.display_name(), "synthetic.csv");
    assert_eq!(now.prepared().unwrap().counts.len(), 1);
}

#[test]
fn a_replace_holds_one_copy_built_from_the_original() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let d = table(3, 10, b"a");
    let original = d.schema.fingerprint.clone();
    let a = opened(&slots, d.clone());
    made(&runs, &slots, a, &mean_of(3));
    let (held, gone) = holding(&slots, a);
    drop(held);

    let sum = sum_of(3);
    let v = made(&runs, &slots, a, &sum);
    assert!(v.errors.is_empty(), "never source_already_prepared");
    assert_eq!(slots.cells_held(), 40, "one copy, of one score");
    assert_eq!(slots.held(), 1);
    assert!(gone.upgrade().is_none(), "the first prepared table is gone");

    let now = slots.get(a).unwrap();
    let direct = stats_prep::build(&d, &sum, CAP).expect("the same recipe on the file");
    assert_eq!(now.dataset(), &direct.dataset, "as if made from the file");
    assert_eq!(now.dataset().schema.columns.len(), 4, "one score, not two");
    assert_eq!(now.original_fingerprint(), &original);
    assert_eq!(
        now.dataset().schema.fingerprint.sha256_hex(),
        stats_prep::prepared_fingerprint(&original, &sum).sha256_hex(),
        "over the original's hash, never the first prepared table's"
    );
}

/// The budget counts the prepared table, and a make can drop an older slot. Its
/// own, at the back, is never the one.
#[test]
fn a_make_past_the_budget_drops_the_oldest_and_never_its_own_slot() {
    let runs = Runs::default();
    let slots = DatasetSlots::with_cell_budget(60);
    let a = opened(&slots, table(3, 10, b"a"));
    let b = opened(&slots, table(3, 10, b"b"));
    assert_eq!(slots.order(), [a, b], "60 of 60");

    made(&runs, &slots, a, &mean_of(3));
    assert_eq!(slots.order(), [a], "70 passes 60: the other slot goes");
    assert_eq!(slots.get(b).err(), Some(RunCode::UnknownInput));

    let alone = DatasetSlots::with_cell_budget(30);
    let c = opened(&alone, table(3, 10, b"c"));
    made(&runs, &alone, c, &mean_of(3));
    assert_eq!(alone.order(), [c], "alone and over the budget, it stays");
    assert_eq!(alone.get(c).unwrap().dataset().schema.columns.len(), 4);
}

/* ---- a refused make changes nothing ------------------------------------------ */

/// The slot itself, not a copy: its table, its recipe, its counts and its place.
fn assert_unchanged(slots: &DatasetSlots, slot: u64, was: &Arc<Slot>, order: &[u64], why: &str) {
    assert!(
        Arc::ptr_eq(&slots.get(slot).unwrap(), was),
        "{why}: the slot"
    );
    assert_eq!(slots.order(), order, "{why}: its place in the queue");
}

#[test]
fn a_refused_make_or_replace_leaves_the_slot_exactly_as_it_was() {
    let runs = Runs::default();
    for already_prepared in [false, true] {
        let slots = DatasetSlots::default();
        let a = opened(&slots, table(3, 10, b"a"));
        let b = opened(&slots, table(3, 10, b"b"));
        if already_prepared {
            made(&runs, &slots, a, &mean_of(3));
            made(&runs, &slots, b, &mean_of(3));
        }
        let order = slots.order();
        let was = slots.get(a).unwrap();
        let bad_name = recipe_of(3, step(3, " untrimmed ", Some(Statistic::Sum)));
        let open_choice = recipe_of(3, step(3, "open", None));
        let wrong_width = mean_of(4);
        let mut no_row = step(3, "none", Some(Statistic::Sum));
        no_row.missing_codes = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let no_row = recipe_of(3, no_row);

        for (recipe, status, why) in [
            (&bad_name, ValidationStatus::Invalid, "an invalid name"),
            (
                &open_choice,
                ValidationStatus::NeedsUserInput,
                "an open choice",
            ),
            (&wrong_width, ValidationStatus::Invalid, "another width"),
            (&no_row, ValidationStatus::Invalid, "a score no row has"),
        ] {
            let v = prep::make(&runs, &slots, a, recipe, CAP)
                .map_err(|_| "a report, not an error")
                .unwrap();
            assert_eq!(v.status, status, "{why}");
            assert!(v.dataset.is_none(), "{why}: no table");
            assert_unchanged(&slots, a, &was, &order, why);
        }
        // A score no row has says so, with its counts.
        let v = prep::make(&runs, &slots, a, &no_row, CAP)
            .map_err(|_| "a report, not an error")
            .unwrap();
        let v = serde_json::to_value(&v).unwrap();
        assert_eq!(v["errors"][0]["code"], "stats.prep.no_row_scored", "{v}");
        assert_eq!(v["counts"][0]["notScoredTooFew"], 10, "{v}");

        // The cells cap, on this table's own numbers.
        let v = prep::make(&runs, &slots, a, &sum_of(3), 39)
            .map_err(|_| "a report, not an error")
            .unwrap();
        assert_eq!(v.status, ValidationStatus::Invalid);
        assert_unchanged(&slots, a, &was, &order, "the cells cap");

        // A recipe of another capability: the registry's own refusal.
        let mut other = sum_of(3);
        other.capability_id = CapabilityId::parse("stats.describe").unwrap();
        assert!(prep::make(&runs, &slots, a, &other, CAP).is_err());
        assert_unchanged(&slots, a, &was, &order, "another capability");
    }
}

/// A make holds no lock while it builds, so a slot it read may have changed by
/// the end. Then it must not overwrite what is there.
#[test]
fn a_make_overtaken_by_another_or_by_a_close_changes_nothing() {
    let runs = Runs::default();
    let slots = DatasetSlots::default();
    let a = opened(&slots, table(3, 10, b"a"));
    let (stale, _) = holding(&slots, a);
    let built = stats_prep::build(stale.dataset(), &mean_of(3), CAP).unwrap();
    let rebuilt = stats_prep::build(stale.dataset(), &sum_of(3), CAP).unwrap();

    made(&runs, &slots, a, &sum_of(3));
    let (now, _) = holding(&slots, a);
    assert_eq!(
        slots.hold_prepared(&stale, built).err(),
        Some(RunCode::RunBusy),
        "another make got there first"
    );
    assert!(
        Arc::ptr_eq(&slots.get(a).unwrap(), &now),
        "what it held stays"
    );

    assert!(slots.close(a));
    assert_eq!(
        slots.hold_prepared(&now, rebuilt).err(),
        Some(RunCode::UnknownInput),
        "closed meanwhile"
    );
    assert_eq!(slots.held(), 0, "nothing came back");
    assert!(matches!(
        prep::make(&runs, &slots, a, &mean_of(3), CAP),
        Err(_)
    ));
}

/* ---- the bytes, the fingerprint, the findings ---------------------------------- */

/// A Save holds the slot (project_commands.rs:1491), so the slot is never taken
/// apart: the new one shares the bytes, and the held one is still whole.
#[test]
fn a_make_while_a_handle_is_held_shares_the_bytes_and_leaves_that_handle_whole() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let d = table(3, 10, b"a");
    let a = opened(&slots, d.clone());
    let save_holds = slots.get(a).unwrap();
    let bytes_at = save_holds.source().unwrap().bytes().as_ptr();

    made(&runs, &slots, a, &mean_of(3));
    let now = slots.get(a).unwrap();
    assert_eq!(
        now.source().unwrap().bytes().as_ptr(),
        bytes_at,
        "the same bytes, not a copy"
    );
    assert_eq!(now.source().unwrap().bytes(), BYTES);
    assert_eq!(save_holds.dataset(), &d, "the held handle keeps its table");
    assert!(save_holds.prepared().is_none());
    assert_eq!(save_holds.source().unwrap().bytes(), BYTES);

    desc_clear(&runs, &slots, a);
    assert_eq!(
        slots.get(a).unwrap().source().unwrap().bytes().as_ptr(),
        bytes_at,
        "and a clear keeps them"
    );
}

fn desc_clear(runs: &Runs, slots: &DatasetSlots, slot: u64) -> desc::DatasetView {
    prep::clear(runs, slots, slot)
        .map_err(|_| "clear refused")
        .unwrap()
}

#[test]
fn a_slot_with_no_source_file_can_take_a_recipe() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let d = table(3, 10, b"a");
    let original = d.schema.fingerprint.clone();
    let a = slots.insert("synthetic.xlsx".into(), d);
    let v = made(&runs, &slots, a, &mean_of(3));
    assert!(v.dataset.is_some());
    let now = slots.get(a).unwrap();
    assert!(now.source().is_none());
    let p = now.preparation().expect("a preparation");
    assert_eq!(p.source_hash.as_hex(), original.sha256_hex());
    let view = desc_clear(&runs, &slots, a);
    assert_eq!(view.input_ref.content_hash.as_hex(), original.sha256_hex());
}

#[test]
fn a_clear_gives_back_the_file_table_its_fingerprint_and_the_opens_view() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let d = table(3, 10, b"a");
    let a = opened(&slots, d.clone());
    let was = view_bytes(&desc::view_of(&slots.get(a).unwrap()));

    made(&runs, &slots, a, &mean_of(3));
    let (held, gone) = holding(&slots, a);
    drop(held);
    let cleared = desc_clear(&runs, &slots, a);
    assert_eq!(
        view_bytes(&cleared),
        was,
        "the view's bytes, inputRef included"
    );
    let now = slots.get(a).unwrap();
    assert_eq!(now.dataset(), &d, "cells, schema and fingerprint");
    assert_eq!(now.original_fingerprint(), &d.schema.fingerprint);
    assert!(now.prepared().is_none() && now.preparation().is_none());
    assert!(gone.upgrade().is_none(), "the prepared table is gone");
    assert_eq!(slots.cells_held(), 30);

    // A recipe after a clear is made from the original as the first was.
    let again = made(&runs, &slots, a, &mean_of(3));
    assert_eq!(
        serde_json::to_value(&again.dataset).unwrap()["inputRef"],
        serde_json::to_value(stats_desc::input_ref(
            &stats_prep::build(&d, &mean_of(3), CAP).unwrap().dataset
        ))
        .unwrap()
    );
}

#[test]
fn a_clear_with_no_recipe_leaves_the_slot_as_it_is() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let a = opened(&slots, table(3, 10, b"a"));
    let b = opened(&slots, table(3, 10, b"b"));
    let was = slots.get(a).unwrap();
    let v = desc_clear(&runs, &slots, a);
    assert_eq!(v.slot, a);
    assert!(Arc::ptr_eq(&slots.get(a).unwrap(), &was));
    assert_eq!(slots.order(), [a, b]);
}

#[test]
fn the_files_findings_survive_a_make_a_replace_and_a_clear() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let finding = SavFinding::new(SavFindingCode::CaseSizeMissing, &[(SavParam::Used, 3.0)]);
    let a = slots.insert_opened(
        "synthetic.sav".into(),
        table(3, 10, b"a"),
        None,
        vec![finding.clone()],
    );
    let v = made(&runs, &slots, a, &mean_of(3));
    assert_eq!(v.dataset.unwrap().file_findings, [finding.clone()]);
    let v = made(&runs, &slots, a, &sum_of(3));
    assert_eq!(v.dataset.unwrap().file_findings, [finding.clone()]);
    assert_eq!(desc_clear(&runs, &slots, a).file_findings, [finding]);
}

/* ---- the view ------------------------------------------------------------------ */

#[test]
fn the_view_marks_the_score_and_names_it_and_a_files_column_keeps_its_bytes() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let d = table(3, 10, b"a");
    let a = opened(&slots, d.clone());
    let before = view_bytes(&desc::view_of(&slots.get(a).unwrap()));
    let recipe = recipe_of(3, step(3, "wellbeing", Some(Statistic::Mean)));
    let v = made(&runs, &slots, a, &recipe);
    let view = view_bytes(v.dataset.as_ref().unwrap());

    let columns = view["profile"]["columns"].as_array().unwrap();
    assert_eq!(columns.len(), 4, "the profile of the whole table");
    assert_eq!(columns[3]["prepared"], json!(true));
    assert_eq!(columns[3]["scoreName"], "wellbeing");
    assert_eq!(columns[3]["alias"], "C4");
    assert_eq!(columns[3]["header"], "C4", "the name is never the header");
    assert_eq!(columns[3]["kind"], "numeric");
    for i in 0..3 {
        assert_eq!(
            columns[i], before["profile"]["columns"][i],
            "a file's column has the same bytes on the prepared table"
        );
        assert!(columns[i].get("prepared").is_none());
        assert!(columns[i].get("scoreName").is_none());
    }
    assert!(before["profile"]["columns"][0].get("prepared").is_none());
    assert!(
        before.get("preparation").is_none(),
        "absent on the file's table"
    );
    assert!(before.get("scoreCounts").is_none());

    let counts = &view["scoreCounts"][0];
    assert_eq!(counts["column"], "C4");
    assert_eq!(counts["scoredFromEveryItem"], 10);
    assert_eq!(
        serde_json::to_value(&v.counts).unwrap()[0],
        view["scoreCounts"][0],
        "the report's counts are the view's"
    );

    // It is the core's profile of the whole table, the overlay aside.
    let held = slots.get(a).unwrap();
    let core = serde_json::to_value(stats_desc::profile(held.dataset())).unwrap();
    let mut plain = columns.clone();
    let last = plain[3].as_object_mut().unwrap();
    last.remove("prepared");
    last.remove("scoreName");
    assert_eq!(Value::Array(plain), core["columns"]);
}

/// The preparation is what every method checks: its hash over the original's
/// and the recipe's gives this table's input.
#[test]
fn the_preparation_is_built_from_the_original_and_gives_the_tables_input() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let d = table(3, 10, b"a");
    let a = opened(&slots, d.clone());
    let recipe = mean_of(3);
    let v = made(&runs, &slots, a, &recipe);
    let view = v.dataset.expect("the view");
    let p = view.preparation.as_ref().expect("a preparation");
    assert_eq!(p.source_hash.as_hex(), d.schema.fingerprint.sha256_hex());
    assert_eq!(p.recipe_hash.as_hex(), stats_prep::recipe_hash(&recipe));
    assert_eq!(p.source_columns, 3);
    assert_eq!(p.capability_version, stats_prep::CAPABILITY_VERSION);
    assert_eq!(p.engine_version, stats_prep::ENGINE_VERSION);
    assert_eq!(
        stats_prep::prepared_input_hash(p.source_hash.as_hex(), p.recipe_hash.as_hex()),
        view.input_ref.content_hash.as_hex()
    );
    assert_ne!(
        p.source_hash.as_hex(),
        view.input_ref.content_hash.as_hex(),
        "the source's hash is the original's, never the prepared table's"
    );
    assert_eq!(slots.get(a).unwrap().preparation().as_ref(), Some(p));
}

/* ---- each tier's cap ------------------------------------------------------------- */

/// The largest table a tier's cap allows, and one more cell: a file at the
/// 8 GB cap takes no score, and the same table on 16 GB does.
#[test]
fn each_tiers_cells_cap_is_what_the_make_is_given() {
    let runs = Runs::default();
    let cap_8 = IngestLimits::for_tier(MemoryTier::Gb8).max_cells as u64;
    let cap_16 = IngestLimits::for_tier(MemoryTier::Gb16).max_cells as u64;
    let cap_32 = IngestLimits::for_tier(MemoryTier::Gb32).max_cells as u64;
    assert_eq!((cap_8, cap_16, cap_32), (5_000_000, 12_800_000, 12_800_000));

    let slots = DatasetSlots::default();
    let at_the_cap = slots.insert("wide".into(), table(50, 100_000, b"wide"));
    let was = slots.get(at_the_cap).unwrap();
    let v = prep::make(&runs, &slots, at_the_cap, &mean_of(50), cap_8)
        .map_err(|_| "a report")
        .unwrap();
    assert_eq!(v.status, ValidationStatus::Invalid);
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json["errors"][0]["code"], "stats.prep.cells_over_cap");
    assert_eq!(json["errors"][0]["params"]["found"], 5_100_000);
    assert_eq!(json["errors"][0]["params"]["limit"], 5_000_000);
    assert!(Arc::ptr_eq(&slots.get(at_the_cap).unwrap(), &was));
    drop(was);

    for cap in [cap_16, cap_32] {
        let v = prep::make(&runs, &slots, at_the_cap, &mean_of(50), cap)
            .map_err(|_| "a report")
            .unwrap();
        assert!(v.dataset.is_some(), "{cap}");
        assert_eq!(slots.cells_held(), 5_100_000, "{cap}: counted once");
    }

    // A table one column narrower reaches the 8 GB cap exactly, and passes it not.
    let exactly = DatasetSlots::default();
    let s = exactly.insert("narrow".into(), table(49, 100_000, b"narrow"));
    let v = prep::make(&runs, &exactly, s, &mean_of(49), cap_8)
        .map_err(|_| "a report")
        .unwrap();
    assert!(
        v.dataset.is_some(),
        "5,000,000 cells is the cap, not past it"
    );
    assert_eq!(exactly.cells_held(), 5_000_000);
}

/* ---- the CSV ------------------------------------------------------------------------ */

#[test]
fn the_csv_is_the_cores_from_the_one_copy_with_its_hash_and_a_slot_with_no_recipe_has_none() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let a = opened(&slots, table(3, 10, b"a"));
    let e = prep::csv(&slots, a).map(|_| ()).unwrap_err();
    assert!(e.to_string().contains("run.unknown_input"), "{e}");

    made(&runs, &slots, a, &mean_of(3));
    let (held, _) = holding(&slots, a);
    let v = prep::csv(&slots, a).map_err(|_| "csv").unwrap();
    assert_eq!(
        v.bytes,
        stats_prep::export::prepared_csv(held.prepared().unwrap()),
        "the core's bytes"
    );
    assert_eq!(v.sha256, ContentHash::of_bytes(&v.bytes).as_hex());
    assert!(
        Arc::ptr_eq(&slots.get(a).unwrap(), &held),
        "and nothing changes"
    );
    assert!(String::from_utf8(v.bytes)
        .unwrap()
        .starts_with("\"q1\",\"q2\",\"q3\",\"a mean\"\r\n"));
}

/// A number never goes through the text writer: its guard includes `-`.
#[test]
fn a_negative_score_and_item_reach_the_csv_unquoted() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let d = build_dataset(
        RawTable::from_rows(
            vec!["a".into(), "b".into()],
            vec![
                vec!["-1.5".into(), "-2.5".into()],
                vec!["1".into(), "2".into()],
            ],
        ),
        DataFingerprint::of_bytes(b"negative", "test", TableRef::FirstTable),
    )
    .unwrap();
    let a = opened(&slots, d);
    let recipe = recipe_of(2, step(2, "total", Some(Statistic::Sum)));
    made(&runs, &slots, a, &recipe);
    let text = String::from_utf8(prep::csv(&slots, a).map_err(|_| "csv").unwrap().bytes).unwrap();
    assert_eq!(text, "\"a\",\"b\",\"total\"\r\n-1.5,-2.5,-4\r\n1,2,3\r\n");
}

/// A reversed item, read back: the build ran on the ends the recipe gives,
/// through the same make.
#[test]
fn a_reversed_score_is_built_through_the_make() {
    let (runs, slots) = (Runs::default(), DatasetSlots::default());
    let d = table(3, 10, b"a");
    let a = opened(&slots, d);
    let mut s = step(3, "reversed", Some(Statistic::Sum));
    s.reversed = vec![alias(2)];
    s.scale = Some(ScaleEnds { min: 1.0, max: 5.0 });
    let v = made(&runs, &slots, a, &recipe_of(3, s));
    assert_eq!(v.status, ValidationStatus::Valid);
    assert!(v.dataset.is_some());
}
