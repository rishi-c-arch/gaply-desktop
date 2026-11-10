//! **`gaply_core::stats_ranks`'s closed outcomes, its check point and its
//! refusals (Research OS, Phase 8Y-1, decisions 602 and 604).** The numbers
//! are `independent_ranks_r_golden.rs`'s; this file pins what is decided
//! rather than computed: every refusal, exclusion and note by code, the
//! exclusions' order, the levels' order, the test chosen by the count of
//! included levels, the exact p given and absent by rule, the check point,
//! and the result's exact keys.

use std::collections::BTreeSet;
use std::ops::ControlFlow;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{
    CapabilityId, Diagnostic, OperationId, ParamKey, ParamValue, ValidationStatus, WorkspaceId,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_ranks::{
    check_points, compute, run, spec, validate, Halt, IndependentRanksOutcome,
    IndependentRanksParams, KruskalWallis, MannWhitney, RankTest, RanksCode, Stat, CAPABILITY_ID,
    CAPABILITY_VERSION, ENGINE_VERSION, EXACT_BELOW, GROUP_NOTE_BELOW,
};

fn raw(rows: &[(String, String)]) -> RawTable {
    RawTable::from_rows(
        vec!["y".into(), "g".into()],
        rows.iter().map(|(y, g)| vec![y.clone(), g.clone()]).collect(),
    )
}

fn dataset(rows: &[(String, String)]) -> Dataset {
    build_dataset(
        raw(rows),
        DataFingerprint::of_bytes(format!("{rows:?}").as_bytes(), "ranks cases", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// Rows of (outcome, group) from string slices.
fn rows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(y, g)| (y.to_string(), g.to_string())).collect()
}

/// `values` in `group`, each written as Rust writes the double.
fn group(group: &str, values: impl IntoIterator<Item = f64>) -> Vec<(String, String)> {
    values.into_iter().map(|v| (format!("{v}"), group.to_string())).collect()
}

const OUTCOME: Alias = Alias(ColumnAlias(1));
const GROUP: Alias = Alias(ColumnAlias(2));

fn params(levels: Option<Vec<u32>>, codes: &[f64]) -> IndependentRanksParams {
    IndependentRanksParams {
        outcome: OUTCOME,
        group: GROUP,
        levels,
        missing_codes: codes.to_vec(),
    }
}

fn all() -> IndependentRanksParams {
    params(None, &[])
}

fn refusal(d: &Dataset, p: IndependentRanksParams) -> Diagnostic<RanksCode> {
    compute(d, &p).expect_err("refused")
}

fn int(e: &Diagnostic<RanksCode>, key: ParamKey) -> Option<i64> {
    match e.params.get(&key) {
        Some(ParamValue::Int(v)) => Some(*v),
        _ => None,
    }
}

fn mann_whitney(r: &IndependentRanksOutcome) -> MannWhitney {
    match r.test {
        RankTest::MannWhitney(t) => t,
        RankTest::KruskalWallis(_) => panic!("the Kruskal-Wallis test where Mann-Whitney's was due"),
    }
}

fn kruskal_wallis(r: &IndependentRanksOutcome) -> KruskalWallis {
    match r.test {
        RankTest::KruskalWallis(t) => t,
        RankTest::MannWhitney(_) => panic!("the Mann-Whitney test where Kruskal-Wallis's was due"),
    }
}

/// Two groups of untied values: a 1.5, 3.5, 5.5 and b 2.5, 4.5, 6.5, 7.5.
fn base() -> Vec<(String, String)> {
    let mut d = group("a", [1.5, 3.5, 5.5]);
    d.extend(group("b", [2.5, 4.5, 6.5, 7.5]));
    d
}

/// `n` untied values from `from`, a step of 1 apart.
fn spread(from: f64, n: usize) -> Vec<f64> {
    (0..n).map(|i| from + i as f64).collect()
}

#[test]
fn the_capability_is_stats_analyze_independent_ranks_version_1_engine_1() {
    assert_eq!(CAPABILITY_ID, "stats.analyze.independent_ranks");
    assert_eq!((CAPABILITY_VERSION, ENGINE_VERSION), (1, 1));
    assert_eq!((GROUP_NOTE_BELOW, EXACT_BELOW), (5, 50));
    assert_eq!(check_points(), 1);
}

#[test]
fn every_code_is_a_stats_ranks_word_and_holds_no_verdict_word() {
    for c in RanksCode::ALL {
        let w = c.wire();
        assert!(w.starts_with("stats.ranks."), "{w}");
        let words: Vec<&str> = w["stats.ranks.".len()..].split('_').collect();
        for bad in ["should", "recommend", "valid", "accurate"] {
            assert!(!words.contains(&bad), "{w} holds {bad}");
        }
        assert!(!w.contains("significan"), "{w}");
    }
}

#[test]
fn a_missing_code_is_refused_by_index_when_not_finite() {
    let d = dataset(&base());
    let e = refusal(&d, params(None, &[f64::NAN]));
    assert_eq!((e.code, int(&e, ParamKey::Index)), (RanksCode::MissingCodeNotFinite, Some(0)));
    let e = refusal(&d, params(None, &[1.0, f64::INFINITY]));
    assert_eq!((e.code, int(&e, ParamKey::Index)), (RanksCode::MissingCodeNotFinite, Some(1)));
    // Codes in any order, repeated or not, are taken as given.
    assert!(compute(&d, &params(None, &[9.0, 1.0, 9.0])).is_ok());
}

#[test]
fn the_column_refusals_name_the_column_by_alias_never_a_label() {
    let d = dataset(&base());
    let p = IndependentRanksParams {
        outcome: Alias(ColumnAlias(7)),
        ..all()
    };
    let e = refusal(&d, p);
    assert_eq!((e.code, int(&e, ParamKey::Column)), (RanksCode::UnknownColumn, Some(7)));
    let p = IndependentRanksParams {
        group: Alias(ColumnAlias(8)),
        ..all()
    };
    let e = refusal(&d, p);
    assert_eq!((e.code, int(&e, ParamKey::Column)), (RanksCode::UnknownColumn, Some(8)));
    let p = IndependentRanksParams {
        group: OUTCOME,
        ..all()
    };
    assert_eq!(refusal(&d, p).code, RanksCode::SameColumn);
    let d = dataset(&rows(&[("1", "a"), ("x", "b"), ("2", "a"), ("words", "b"), ("3", "b")]));
    let e = refusal(&d, all());
    assert_eq!(
        (e.code, int(&e, ParamKey::Column), int(&e, ParamKey::Count)),
        (RanksCode::OutcomeHoldsText, Some(1), Some(2))
    );
    let d = dataset(&rows(&[("1", "a"), ("-inf", "b"), ("2", "a"), ("x", "b"), ("3", "b")]));
    let e = refusal(&d, all());
    assert_eq!(
        (e.code, int(&e, ParamKey::Column), int(&e, ParamKey::Count)),
        (RanksCode::OutcomeNonFinite, Some(1), Some(1))
    );
    for p in e.params.values() {
        assert!(matches!(p, ParamValue::Int(_)), "a count or an alias, never a label: {p:?}");
    }
}

#[test]
fn the_group_takes_two_to_fifty_levels_and_the_ticked_levels_are_checked() {
    let d = dataset(&group("only", spread(1.0, 6)));
    let e = refusal(&d, all());
    assert_eq!(
        (e.code, int(&e, ParamKey::Column), int(&e, ParamKey::Count)),
        (RanksCode::GroupOneLevel, Some(2), Some(1))
    );
    let fifty_one: Vec<(String, String)> =
        (0..102).map(|i| (format!("{i}"), format!("L{:02}", i % 51))).collect();
    let e = refusal(&dataset(&fifty_one), all());
    assert_eq!(
        (e.code, int(&e, ParamKey::Count), int(&e, ParamKey::Limit)),
        (RanksCode::TooManyLevels, Some(51), Some(50))
    );
    let fifty: Vec<(String, String)> =
        (0..100).map(|i| (format!("{i}"), format!("L{:02}", i % 50))).collect();
    let r = compute(&dataset(&fifty), &all()).expect("fifty levels are taken");
    assert_eq!(kruskal_wallis(&r).df, 49.0);
    let mut three = base();
    three.extend(group("c", [8.5, 9.5]));
    let d = dataset(&three);
    let e = refusal(&d, params(Some(vec![0, 3]), &[]));
    assert_eq!(
        (e.code, int(&e, ParamKey::Index), int(&e, ParamKey::Limit)),
        (RanksCode::LevelOutOfRange, Some(3), Some(2))
    );
    let e = refusal(&d, params(Some(vec![2, 0, 2]), &[]));
    assert_eq!((e.code, int(&e, ParamKey::Index)), (RanksCode::LevelRepeated, Some(2)));
    let e = refusal(&d, params(Some(vec![1]), &[]));
    assert_eq!((e.code, int(&e, ParamKey::Count)), (RanksCode::TooFewLevels, Some(1)));
    let e = refusal(&d, params(Some(vec![]), &[]));
    assert_eq!((e.code, int(&e, ParamKey::Count)), (RanksCode::TooFewLevels, Some(0)));
}

#[test]
fn an_included_group_with_no_value_is_refused_by_its_index_only_when_nothing_else_is() {
    // b's rows: an empty outcome and the code -9.
    let mut d = group("a", [1.0, 2.0, 3.0]);
    d.extend(rows(&[("", "b"), ("-9", "b")]));
    d.extend(group("c", [4.0, 5.0]));
    let d = dataset(&d);
    let e = refusal(&d, params(None, &[-9.0]));
    assert_eq!(e.code, RanksCode::GroupEmpty);
    assert_eq!(int(&e, ParamKey::Index), Some(1));
    assert_eq!(e.params.len(), 1, "its index only: {:?}", e.params);
    // Left out, b refuses nothing.
    assert!(compute(&d, &params(Some(vec![0, 2]), &[-9.0])).is_ok());
    // With another error, group_empty is not looked for.
    let report = validate(&d, &spec(OperationId(1), &d, params(Some(vec![0, 1, 1]), &[-9.0])));
    let codes: Vec<RanksCode> = report.errors.iter().map(|e| e.code).collect();
    assert_eq!(codes, [RanksCode::LevelRepeated]);
    // A group of one value is a group.
    let mut one = group("a", [1.0]);
    one.extend(group("b", spread(2.0, 30)));
    assert!(compute(&dataset(&one), &all()).is_ok());
}

#[test]
fn the_test_is_chosen_by_the_count_of_included_levels() {
    let mut d = group("d1", [5.5, 6.1, 4.2, 7.3]);
    d.extend(group("d2", [3.1, 2.7, 4.4, 3.9]));
    d.extend(group("d3", [8.8, 9.1, 7.7, 8.2]));
    d.extend(group("d4", [1.0, 1.9, 2.3, 0.8]));
    let d = dataset(&d);
    let r = compute(&d, &params(Some(vec![3, 1]), &[])).unwrap();
    let t = mann_whitney(&r);
    assert_eq!((t.first, t.second), (1, 3), "the first group is the lower index");
    assert_eq!(r.groups.iter().map(|g| g.level).collect::<Vec<_>>(), [1, 3]);
    let r = compute(&d, &params(Some(vec![0, 1, 3]), &[])).unwrap();
    assert_eq!(kruskal_wallis(&r).df, 2.0);
    let r = compute(&d, &all()).unwrap();
    assert_eq!(kruskal_wallis(&r).df, 3.0);
    assert!(r.notes.contains(&RanksCode::GroupBelowFive));
}

#[test]
fn the_groups_are_in_code_point_order_so_10_is_before_2_and_first() {
    let mut d = group("2", [4.1, 5.2, 3.3, 6.4, 5.5, 7.1]);
    d.extend(group("10", [1.1, 2.2, 3.4, 0.5, 2.6, 1.7, 2.9]));
    let r = compute(&dataset(&d), &all()).unwrap();
    let labels: Vec<&str> = r.groups.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(labels, ["10", "2"]);
    let t = mann_whitney(&r);
    assert_eq!((t.first, t.second), (0, 1));
    // 10's values are the smaller: U1 is 1 of 42, z and the rank-biserial
    // correlation negative.
    assert_eq!((t.u1, t.u2), (1.0, 41.0));
    assert!(matches!(t.z, Stat::Value(z) if z < 0.0));
    assert!(t.rank_biserial < 0.0);
    let mut text = Vec::new();
    for (i, l) in ["Zebra", "apple", "Äpfel", "éclair", "Ölung", "banana"].iter().enumerate() {
        text.extend(group(l, spread(i as f64 * 10.0, 3)));
    }
    let r = compute(&dataset(&text), &all()).unwrap();
    let labels: Vec<&str> = r.groups.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(labels, ["Zebra", "apple", "banana", "Äpfel", "Ölung", "éclair"]);
}

#[test]
fn a_missing_code_meets_the_outcome_only_never_the_group() {
    let mut d = group("a", [1.0, 2.0, 3.0, 99.0]);
    d.extend(group("99", [4.0, 5.0, -9.0]));
    let d = dataset(&d);
    let r = compute(&d, &params(None, &[-9.0, 99.0])).unwrap();
    let labels: Vec<&str> = r.groups.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(labels, ["99", "a"], "a group cell of 99 is a level");
    assert_eq!(r.groups.iter().map(|g| g.n).collect::<Vec<_>>(), [2, 3]);
    assert_eq!(r.exclusions.len(), 1);
    assert_eq!(
        (r.exclusions[0].code, r.exclusions[0].count),
        (RanksCode::MissingCode, 2)
    );
    assert_eq!(
        r.exclusions[0].params.get(&ParamKey::Column),
        Some(&ParamValue::Int(1)),
        "found in the outcome"
    );
}

#[test]
fn a_row_is_left_out_once_by_the_first_reason_in_order() {
    let mut d = base();
    d.extend(group("c", [9.0]));
    d.extend(rows(&[
        ("1", ""),   // missing_group
        ("", ""),    // missing_group, once
        ("2", "c"),  // other_level
        ("", "c"),   // other_level, once
        ("-9", "c"), // other_level, once
        ("", "a"),   // missing_outcome
        ("NaN", "b"),// missing_outcome
        ("-9", "a"), // missing_code
        ("-9", "b"), // missing_code
        ("-9", "b"), // missing_code
    ]));
    let r = compute(&dataset(&d), &params(Some(vec![0, 1]), &[-9.0])).unwrap();
    let got: Vec<(RanksCode, u64, Option<&ParamValue<RanksCode>>)> = r
        .exclusions
        .iter()
        .map(|e| (e.code, e.count, e.params.get(&ParamKey::Column)))
        .collect();
    assert_eq!(
        got,
        [
            (RanksCode::MissingGroup, 2, Some(&ParamValue::Int(2))),
            (RanksCode::OtherLevel, 4, Some(&ParamValue::Int(2))),
            (RanksCode::MissingOutcome, 2, Some(&ParamValue::Int(1))),
            (RanksCode::MissingCode, 3, Some(&ParamValue::Int(1))),
        ]
    );
    assert_eq!(r.n, 7);
    assert_eq!(r.n + got.iter().map(|g| g.1).sum::<u64>(), 18, "every row once");
}

#[test]
fn the_exact_p_is_given_without_ties_below_fifty_a_group_and_absent_otherwise() {
    // 49 and 49, untied: given, no note.
    let mut d = group("p", spread(0.0, 49).into_iter().map(|v| v * 2.0));
    d.extend(group("q", spread(0.0, 49).into_iter().map(|v| v * 2.0 + 1.0)));
    let r = compute(&dataset(&d), &all()).unwrap();
    assert!(mann_whitney(&r).exact_p.is_some());
    assert!(r.notes.is_empty(), "{:?}", r.notes);
    // 50 and 10, untied: absent, by size.
    let mut d = group("big", spread(0.0, 50));
    d.extend(group("small", spread(100.0, 10)));
    let r = compute(&dataset(&d), &all()).unwrap();
    assert_eq!(mann_whitney(&r).exact_p, None);
    assert_eq!(r.notes, [RanksCode::ExactNotGivenSize]);
    // One tie: absent, by the tie.
    let mut d = base();
    d.extend(group("a", [4.5]));
    let r = compute(&dataset(&d), &all()).unwrap();
    assert_eq!(mann_whitney(&r).exact_p, None);
    assert_eq!(r.notes[0], RanksCode::ExactNotGivenTies);
    assert_eq!(r.tie_runs, 1);
    // Both: both notes, the ties' first.
    let mut d = group("big", spread(0.0, 50));
    d.extend(group("small", [0.0, 100.0, 101.0]));
    let r = compute(&dataset(&d), &all()).unwrap();
    assert_eq!(
        r.notes,
        [RanksCode::ExactNotGivenTies, RanksCode::ExactNotGivenSize, RanksCode::GroupBelowFive]
    );
    // -0 and 0 are one value: a tie.
    let d = dataset(&rows(&[
        ("-0", "a"),
        ("1", "a"),
        ("2", "a"),
        ("0", "b"),
        ("3", "b"),
        ("4", "b"),
    ]));
    let r = compute(&d, &all()).unwrap();
    assert_eq!((r.tie_runs, mann_whitney(&r).exact_p), (1, None));
    // Three groups: never an exact p, never its notes.
    let mut d = base();
    d.extend(group("c", [1.5, 1.5]));
    let r = compute(&dataset(&d), &all()).unwrap();
    assert!(!r.notes.contains(&RanksCode::ExactNotGivenTies));
    assert!(matches!(r.test, RankTest::KruskalWallis(_)));
}

#[test]
fn the_exact_p_is_capped_at_one_where_twice_a_tail_is_above_it() {
    let d = dataset(&rows(&[("1", "a"), ("4", "a"), ("2", "b"), ("3", "b")]));
    let r = compute(&d, &all()).unwrap();
    let t = mann_whitney(&r);
    assert_eq!(t.u1, 2.0, "U1 at n1 n2 / 2");
    assert_eq!(t.exact_p, Some(1.0));
}

#[test]
fn every_value_equal_leaves_z_h_and_their_p_not_computable_and_u_given() {
    let d = dataset(&group("a", [4.5; 4]).into_iter().chain(group("b", [4.5; 5])).collect::<Vec<_>>());
    let r = compute(&d, &all()).unwrap();
    let t = mann_whitney(&r);
    let nc = Stat::NotComputable(RanksCode::AllValuesTied);
    assert_eq!((t.z, t.p), (nc, nc));
    assert_eq!((t.u1, t.u2, t.rank_biserial, t.exact_p), (10.0, 10.0, 0.0, None));
    assert_eq!(r.groups.iter().map(|g| g.mean_rank).collect::<Vec<_>>(), [5.0, 5.0]);
    assert!(r.notes.contains(&RanksCode::ExactNotGivenTies));
    let d = dataset(
        &["a", "b", "c"]
            .iter()
            .flat_map(|g| group(g, [7.0; 4]))
            .collect::<Vec<_>>(),
    );
    let r = compute(&d, &all()).unwrap();
    let t = kruskal_wallis(&r);
    assert_eq!((t.h, t.p, t.epsilon_squared), (nc, nc, nc));
    assert_eq!(t.df, 2.0);
}

#[test]
fn a_group_below_five_values_is_noted_and_five_is_not() {
    let mut d = group("a", spread(0.0, 5));
    d.extend(group("b", spread(10.0, 5)));
    let r = compute(&dataset(&d), &all()).unwrap();
    assert!(!r.notes.contains(&RanksCode::GroupBelowFive), "{:?}", r.notes);
    let mut d = group("a", spread(0.0, 4));
    d.extend(group("b", spread(10.0, 5)));
    let r = compute(&dataset(&d), &all()).unwrap();
    assert_eq!(r.notes, [RanksCode::GroupBelowFive]);
}

#[test]
fn the_rows_order_changes_no_bit() {
    let mut d = base();
    d.extend(group("c", [1.5, 9.25, 2.5]));
    let forward = compute(&dataset(&d), &all()).unwrap();
    d.reverse();
    let backward = compute(&dataset(&d), &all()).unwrap();
    assert_eq!(forward.canonical_bytes(), backward.canonical_bytes());
}

#[test]
fn the_run_stops_at_its_one_check_point_before_the_arithmetic() {
    let d = dataset(&base());
    let mut seen = Vec::new();
    let out = run(&d, &all(), |i| {
        seen.push(i);
        ControlFlow::Break("stop")
    });
    assert_eq!(out, Err(Halt::Stopped { reason: "stop" }));
    assert_eq!(seen, [0]);
    let mut seen = Vec::new();
    let out = run(&d, &all(), |i| {
        seen.push(i);
        ControlFlow::<()>::Continue(())
    });
    assert!(out.is_ok());
    assert_eq!(seen.len(), check_points());
    let mut reached = false;
    let out = run(&d, &params(None, &[f64::NAN]), |_| {
        reached = true;
        ControlFlow::<()>::Continue(())
    });
    assert!(matches!(out, Err(Halt::Refused(_))) && !reached);
}

#[test]
fn the_result_has_exactly_its_keys() {
    let keys = |v: &serde_json::Value| -> BTreeSet<String> {
        v.as_object().unwrap().keys().cloned().collect()
    };
    let want = |k: &[&str]| -> BTreeSet<String> { k.iter().map(|s| s.to_string()).collect() };
    let top = want(&["groups", "n", "tieRuns", "test", "exclusions", "notes", "limitations"]);
    let r = compute(&dataset(&base()), &all()).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.canonical_bytes()).unwrap();
    assert_eq!(keys(&v), top);
    assert_eq!(
        keys(&v["groups"][0]),
        want(&["level", "label", "n", "median", "rankSum", "meanRank"])
    );
    assert_eq!(keys(&v["test"]), want(&["mann_whitney"]));
    assert_eq!(
        keys(&v["test"]["mann_whitney"]),
        want(&["first", "second", "u1", "u2", "z", "p", "exactP", "rankBiserial"])
    );
    assert_eq!(keys(&v["test"]["mann_whitney"]["z"]), want(&["value"]));
    let mut three = base();
    three.extend(group("c", [8.5, 9.5]));
    let r = compute(&dataset(&three), &all()).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.canonical_bytes()).unwrap();
    assert_eq!(keys(&v), top);
    assert_eq!(
        keys(&v["test"]["kruskal_wallis"]),
        want(&["h", "df", "p", "epsilonSquared"])
    );
    assert_eq!(
        r.limitations,
        [
            RanksCode::NoPairwiseComparisons,
            RanksCode::TwoSidedOnly,
            RanksCode::NoLocationShiftEstimate,
            RanksCode::ExactWithoutTiesOnly,
            RanksCode::NoFurtherKSampleTests,
            RanksCode::NoWeights,
            RanksCode::CodePointOrder,
        ]
    );
    // The group labels print as counts.
    let debug = format!("{:?}", r.groups);
    assert!(debug.contains("label_chars") && !debug.contains("\"a\""), "{debug}");
}

#[test]
fn validate_warns_of_suspect_codes_in_the_outcome_and_refuses_other_specs() {
    let mut d = group("a", [1.0, 2.0, 3.0, 99.0, 99.0]);
    d.extend(group("b", [4.0, 5.0, 6.0, 99.0]));
    let d = dataset(&d);
    let report = validate(&d, &spec(OperationId(3), &d, all()));
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let warned: Vec<(RanksCode, Option<i64>, Option<i64>, Option<i64>)> = report
        .warnings
        .iter()
        .map(|w| {
            (
                w.code,
                int(w, ParamKey::Column),
                int(w, ParamKey::Value),
                int(w, ParamKey::Count),
            )
        })
        .collect();
    assert_eq!(warned, [(RanksCode::SuspectMissingCode, Some(1), Some(99), Some(3))]);
    let report = validate(&d, &spec(OperationId(3), &d, params(None, &[99.0])));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let mut other = spec(OperationId(4), &d, all());
    other.capability_id = CapabilityId::parse("stats.analyze.one_way_anova").unwrap();
    other.capability_version = 2;
    other.workspace_id = WorkspaceId::Compute;
    let report = validate(&d, &other);
    assert_eq!(report.status, ValidationStatus::Unsupported);
    let codes: Vec<RanksCode> = report.errors[..2].iter().map(|u| u.code).collect();
    assert_eq!(
        codes,
        [RanksCode::UnknownCapability, RanksCode::CapabilityVersionMismatch]
    );
    let elsewhere = dataset(&base());
    let report = validate(&d, &spec(OperationId(5), &elsewhere, all()));
    assert_eq!(report.errors[0].code, RanksCode::InputMismatch);
    let mut none = spec(OperationId(6), &d, all());
    none.inputs.clear();
    let report = validate(&d, &none);
    assert_eq!(
        (report.errors[0].code, int(&report.errors[0], ParamKey::Count)),
        (RanksCode::InputCount, Some(0))
    );
}

#[test]
fn the_spec_is_camel_case_and_refuses_unknown_fields() {
    let p: IndependentRanksParams = serde_json::from_str(
        r#"{"outcome":"C1","group":"C2","levels":[0,2],"missingCodes":[-9]}"#,
    )
    .unwrap();
    assert_eq!(p, params(Some(vec![0, 2]), &[-9.0]));
    let p: IndependentRanksParams =
        serde_json::from_str(r#"{"outcome":"C1","group":"C2","levels":null,"missingCodes":[]}"#)
            .unwrap();
    assert_eq!(p, all());
    assert!(serde_json::from_str::<IndependentRanksParams>(
        r#"{"outcome":"C1","group":"C2","levels":null,"missingCodes":[],"postHoc":[]}"#
    )
    .is_err());
    assert!(
        serde_json::from_str::<IndependentRanksParams>(r#"{"outcome":"C1","group":"C2","levels":null}"#)
            .is_err(),
        "missingCodes is required"
    );
}
