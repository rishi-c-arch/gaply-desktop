//! **The profile's level list, capped (Research OS, Phase 8W, decisions 566
//! and 568).** A data file's profile keeps a categorical column's first
//! `PROFILE_LEVELS_MAX` (50) levels in code-point order, with their counts;
//! `distinct` still counts every level, so a list shorter than `distinct` says
//! levels were left out. The summary's own result is not capped: it lists
//! every level, and its output hash is the one 0a84ccc7's code gives.
//!
//! Synthetic tables only, built in memory.

use gaply_core::contracts::ContentHash;
use gaply_core::stats_desc::{
    describe, profile, profile_columns_from, Alias, ColumnKind, ColumnProfile, ColumnRequest,
    ColumnResult, DescribeParams, LevelCount, TreatAs, PROFILE_LEVELS_MAX,
};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef, GROUP_LEVEL_LIMIT,
};
use gaply_core::stats_ttest::MAX_GROUP_LEVELS;

fn table(headers: &[&str], rows: Vec<Vec<String>>) -> Dataset {
    let raw = RawTable::from_rows(headers.iter().map(|h| h.to_string()).collect(), rows.clone());
    let bytes: Vec<u8> = format!("{headers:?}{}", rows.len()).into_bytes();
    build_dataset(raw, DataFingerprint::of_bytes(&bytes, "test", TableRef::FirstTable)).expect("dataset")
}

/// A numeric column `x`, then a text column `g` of `k` levels "L00", "L01",
/// …, each in one row, but the last in code-point order in `extra` more rows:
/// the largest count, so a choice by count would keep it.
fn levels_table(k: usize, extra: usize) -> Dataset {
    let mut rows: Vec<Vec<String>> = (0..k).map(|i| vec![format!("{}.5", i % 7), format!("L{i:02}")]).collect();
    for j in 0..extra {
        rows.push(vec![format!("{j}"), format!("L{:02}", k - 1)]);
    }
    table(&["x", "g"], rows)
}

fn expected(k: usize, extra: usize, kept: usize) -> Vec<LevelCount> {
    (0..kept)
        .map(|i| LevelCount {
            label: format!("L{i:02}"),
            count: if i == k - 1 { 1 + extra as u64 } else { 1 },
        })
        .collect()
}

fn column_g(d: &Dataset) -> ColumnProfile {
    profile(d).columns[1].clone()
}

#[test]
fn the_cap_is_the_group_limit_of_level_1_and_the_t_test() {
    assert_eq!(PROFILE_LEVELS_MAX, 50);
    assert_eq!(PROFILE_LEVELS_MAX, MAX_GROUP_LEVELS);
    assert_eq!(PROFILE_LEVELS_MAX, GROUP_LEVEL_LIMIT);
}

#[test]
fn a_column_of_51_levels_keeps_its_first_50_in_code_point_order() {
    let d = levels_table(51, 9);
    let g = column_g(&d);
    assert_eq!(g.kind, ColumnKind::Categorical);
    assert_eq!(g.distinct, 51, "distinct counts every level");
    assert_eq!(g.n, 60);
    let levels = g.levels.expect("a categorical column has a list");
    assert_eq!(levels, expected(51, 9, 50), "the first 50 by code point, not by count");
    assert!(!levels.iter().any(|l| l.label == "L50"), "the 51st, with the largest count, is left out");
    // Each kept count is the column's count of that level; the sum falls short
    // of n by the left-out level's count only.
    assert_eq!(levels.iter().map(|l| l.count).sum::<u64>(), 50);
    assert!(levels.iter().map(|l| l.count).sum::<u64>() < g.n);
}

#[test]
fn a_column_of_exactly_50_levels_is_profiled_as_before() {
    let d = levels_table(50, 3);
    let g = column_g(&d);
    assert_eq!(g.distinct, 50);
    let levels = g.levels.expect("a list");
    assert_eq!(levels, expected(50, 3, 50));
    assert_eq!(levels.iter().map(|l| l.count).sum::<u64>(), g.n, "nothing left out: the counts sum to n");
}

#[test]
fn a_column_of_100000_levels_keeps_50_and_counts_them_all() {
    let rows: Vec<Vec<String>> = (0..100_000).map(|i| vec![format!("v{i:06}")]).collect();
    let d = table(&["v"], rows);
    let c = &profile(&d).columns[0];
    assert_eq!(c.kind, ColumnKind::Categorical);
    assert_eq!(c.distinct, 100_000);
    let levels = c.levels.as_ref().expect("a list");
    assert_eq!(levels.len(), 50);
    assert_eq!(levels[0].label, "v000000");
    assert_eq!(levels[49].label, "v000049");
    assert!(levels.iter().all(|l| l.count == 1));
}

#[test]
fn profile_columns_from_caps_as_profile_does() {
    let d = levels_table(51, 2);
    let whole = profile(&d).columns;
    for first in [0, 1] {
        let from = profile_columns_from(&d, first);
        assert_eq!(from, whole[first..].to_vec(), "from column {first}");
        let g = from.last().expect("g");
        assert_eq!(g.levels.as_ref().map(Vec::len), Some(50), "from column {first}");
        assert_eq!(g.distinct, 51);
    }
}

/// The summary's own result lists every level, and hashes as 0a84ccc7's code
/// hashed it.
#[test]
fn a_summary_still_lists_every_level_and_its_output_hash_is_unchanged() {
    let summarise = |d: &Dataset, column: u32| {
        let params = DescribeParams {
            columns: vec![ColumnRequest {
                alias: Alias(ColumnAlias(column)),
                treat_as: TreatAs::Categorical,
            }],
            missing_codes: Vec::new(),
        };
        describe(d, &params).expect("describes")
    };
    let d51 = levels_table(51, 9);
    let r51 = summarise(&d51, 2);
    let ColumnResult::Categorical(s) = &r51.columns[0] else { panic!("categorical") };
    assert_eq!(s.levels.len(), 51);
    assert_eq!(s.levels[50], LevelCount { label: "L50".into(), count: 10 });

    let rows: Vec<Vec<String>> = (0..100_000).map(|i| vec![format!("v{i:06}")]).collect();
    let big = table(&["v"], rows);
    let r100k = summarise(&big, 1);
    let ColumnResult::Categorical(s) = &r100k.columns[0] else { panic!("categorical") };
    assert_eq!(s.levels.len(), 100_000);

    let hashes = [
        ContentHash::of_bytes(&r51.canonical_bytes()).as_hex().to_string(),
        ContentHash::of_bytes(&r100k.canonical_bytes()).as_hex().to_string(),
    ];
    assert_eq!(hashes, AT_0A84CCC7, "the summary's output hash");
}

/// Taken by this test from 0a84ccc7's stats_desc, before the cap.
const AT_0A84CCC7: [&str; 2] = [
    "05e04fdbb619f21ed4847162bb74e836a0d15f1b1113f615f66dd7643bac1933",
    "420eb7d92260d5944d5a119c1c6e1bab64553de571c3db545c4e776f757848b3",
];
