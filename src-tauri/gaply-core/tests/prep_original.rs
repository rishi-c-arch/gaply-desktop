//! **The file's table rebuilt from a prepared one, and a score's levels
//! refused (Research OS, Phase 8M-3, decisions 424 and 425).**
//!
//! A prepared slot holds one copy of the table. Its original comes back by
//! dropping the appended columns, with no parse, so it must equal what was
//! opened: the same cells, the same schema, the same profile bytes. A changed
//! recipe is built from that, never from a prepared table. A column offered as
//! a group is listed only if the file holds it.

use gaply_core::stats_desc::{self, Alias};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_prep::{
    self, MissingRule, PrepCode, PrepRefusal, Recipe, RecipeStep, ScaleScoreStep, Statistic,
};
use gaply_core::stats_ttest::{self, GroupLevelsRefusal};

const CAP: u64 = 10_000;

fn alias(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

/// Items with a gap, a suspect code and a label column, so every part of the
/// schema has something to carry.
fn opened() -> Dataset {
    let rows: Vec<Vec<String>> = [
        ["1", "2", "a"],
        ["2", "", "b"],
        ["3", "4", "a"],
        ["99", "5", "b"],
        ["5", "1", "a"],
    ]
    .iter()
    .map(|r| r.iter().map(|c| c.to_string()).collect())
    .collect();
    build_dataset(
        RawTable::from_rows(vec!["q1".into(), "q2".into(), "group".into()], rows),
        DataFingerprint::of_bytes(b"the file's own bytes", "test", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn recipe(name: &str, statistic: Statistic) -> Recipe {
    Recipe {
        schema_version: stats_prep::RECIPE_SCHEMA_VERSION,
        capability_id: gaply_core::contracts::CapabilityId::parse(stats_prep::CAPABILITY_ID)
            .unwrap(),
        capability_version: stats_prep::CAPABILITY_VERSION,
        engine_version: stats_prep::ENGINE_VERSION,
        source_columns: 3,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: alias(4),
            name: name.into(),
            items: vec![alias(1), alias(2)],
            reversed: Vec::new(),
            scale: None,
            statistic: Some(statistic),
            rule: Some(MissingRule::EveryItem),
            missing_codes: Vec::new(),
        })],
    }
}

#[test]
fn the_original_rebuilt_equals_the_dataset_as_opened_and_its_profile_bytes() {
    let d = opened();
    let prepared = stats_prep::build(&d, &recipe("total", Statistic::Sum), CAP).expect("builds");
    assert_ne!(prepared.dataset, d, "the prepared table has a column more");

    let back = stats_prep::original(&prepared.dataset, d.schema.fingerprint.clone());
    assert_eq!(back, d, "cells, schema and fingerprint");
    assert_eq!(
        serde_json::to_vec(&stats_desc::profile(&back)).unwrap(),
        serde_json::to_vec(&stats_desc::profile(&d)).unwrap(),
        "the profile's bytes"
    );
    assert_eq!(stats_desc::input_ref(&back), stats_desc::input_ref(&d));
}

/// The fingerprint is the one the caller gives: a prepared table's own would
/// name a recipe no longer applied.
#[test]
fn the_original_takes_the_fingerprint_it_is_given_and_not_the_prepared_tables() {
    let d = opened();
    let prepared = stats_prep::build(&d, &recipe("total", Statistic::Sum), CAP).expect("builds");
    assert_ne!(prepared.dataset.schema.fingerprint, d.schema.fingerprint);
    let other = DataFingerprint::of_bytes(b"another", "test", TableRef::FirstTable);
    let back = stats_prep::original(&prepared.dataset, other.clone());
    assert_eq!(back.schema.fingerprint, other);
}

#[test]
fn a_changed_recipe_builds_from_the_original_and_is_refused_on_a_prepared_table() {
    let d = opened();
    let first = stats_prep::build(&d, &recipe("total", Statistic::Sum), CAP).expect("builds");
    let second = recipe("average", Statistic::Mean);

    let on_prepared = stats_prep::build(&first.dataset, &second, CAP).unwrap_err();
    let PrepRefusal::Report(report) = on_prepared else {
        panic!("a report")
    };
    assert!(
        report
            .errors
            .iter()
            .any(|e| e.code == PrepCode::SourceAlreadyPrepared),
        "{:?}",
        report.errors
    );

    let back = stats_prep::original(&first.dataset, d.schema.fingerprint.clone());
    let rebuilt = stats_prep::build(&back, &second, CAP).expect("builds from the original");
    let direct = stats_prep::build(&d, &second, CAP).expect("builds from the file");
    assert_eq!(rebuilt.dataset, direct.dataset);
    assert_eq!(rebuilt.counts, direct.counts);
}

#[test]
fn a_files_own_table_comes_back_as_a_copy_of_itself() {
    let d = opened();
    assert_eq!(stats_prep::original(&d, d.schema.fingerprint.clone()), d);
}

#[test]
fn a_scores_levels_are_refused_and_a_files_columns_are_listed() {
    let d = opened();
    let prepared = stats_prep::build(&d, &recipe("total", Statistic::Sum), CAP).expect("builds");
    assert_eq!(
        stats_ttest::levels_for_group(&prepared.dataset, alias(4)),
        Err(GroupLevelsRefusal::Score),
        "its values are never listed"
    );
    assert_eq!(
        stats_ttest::levels_for_group(&prepared.dataset, alias(9)),
        Err(GroupLevelsRefusal::NoSuchColumn)
    );
    for table in [&d, &prepared.dataset] {
        assert_eq!(
            stats_ttest::levels_for_group(table, alias(3)).unwrap(),
            stats_ttest::group_levels(table, alias(3)).unwrap(),
            "a file's column, prepared table or not"
        );
    }
    assert_eq!(
        stats_ttest::levels_for_group(&d, alias(4)),
        Err(GroupLevelsRefusal::NoSuchColumn),
        "the file's table has no fourth column"
    );
    // The core's own function is unchanged: it still answers for any column.
    assert!(stats_ttest::group_levels(&prepared.dataset, alias(4)).is_some());
}
