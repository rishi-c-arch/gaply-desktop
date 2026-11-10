//! **`gaply_core::stats_related_ranks`'s closed outcomes, its check point and its
//! refusals (Research OS, Phase 8Z-1, decisions 636 and 638).** The numbers are
//! `related_ranks_r_golden.rs`'s; this file pins what is decided rather than
//! computed: every refusal a table of the file's own columns can reach, exclusion
//! and note by code, their order, the test chosen by the count of columns, the exact p given and absent by rule, the zeros
//! dropped, the column order and the signs, the check point, and the result's exact
//! keys. A score's `code_equals_score` and a `preparation_mismatch` need a prepared
//! table, so they are `prep_methods.rs`'s and `stats_related_ranks_commands.rs`'s.

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
use gaply_core::stats_related_ranks::{
    check_points, compute, run, spec, validate, Friedman, Halt, RelatedRanksCode, RelatedRanksOutcome,
    RelatedRanksParams, RelatedTest, Stat, Wilcoxon, CAPABILITY_ID, CAPABILITY_VERSION,
    ENGINE_VERSION, EXACT_BELOW, MAX_RELATED_COLUMNS, ROWS_NOTE_BELOW,
};

type C = RelatedRanksCode;

/// A table of columns named c1, c2, ….
fn table(columns: &[Vec<String>]) -> Dataset {
    let headers = (1..=columns.len()).map(|i| format!("c{i}")).collect();
    let rows = (0..columns[0].len())
        .map(|r| columns.iter().map(|c| c[r].clone()).collect())
        .collect();
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(format!("{columns:?}").as_bytes(), "related cases", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// Two columns from pairs of cells.
fn pairs(rows: &[(&str, &str)]) -> Dataset {
    table(&[
        rows.iter().map(|r| r.0.to_string()).collect(),
        rows.iter().map(|r| r.1.to_string()).collect(),
    ])
}

/// Two columns of numbers, row by row.
fn numbers(rows: &[(f64, f64)]) -> Dataset {
    table(&[
        rows.iter().map(|r| format!("{}", r.0)).collect(),
        rows.iter().map(|r| format!("{}", r.1)).collect(),
    ])
}

fn alias(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn params(columns: &[u32], codes: &[f64]) -> RelatedRanksParams {
    RelatedRanksParams {
        columns: columns.iter().map(|n| alias(*n)).collect(),
        missing_codes: codes.to_vec(),
    }
}

fn two() -> RelatedRanksParams {
    params(&[1, 2], &[])
}

fn refusal(d: &Dataset, p: RelatedRanksParams) -> Diagnostic<C> {
    compute(d, &p).expect_err("refused")
}

fn int(e: &Diagnostic<C>, key: ParamKey) -> Option<i64> {
    match e.params.get(&key) {
        Some(ParamValue::Int(v)) => Some(*v),
        _ => None,
    }
}

fn wilcoxon(r: &RelatedRanksOutcome) -> Wilcoxon {
    match &r.test {
        RelatedTest::Wilcoxon(t) => *t,
        RelatedTest::Friedman(_) => panic!("the Friedman test where Wilcoxon's was due"),
    }
}

fn friedman(r: &RelatedRanksOutcome) -> Friedman {
    match &r.test {
        RelatedTest::Friedman(t) => t.clone(),
        RelatedTest::Wilcoxon(_) => panic!("the Wilcoxon test where Friedman's was due"),
    }
}

/// `n` pairs, none tied in size and none zero: d = 1, -2, 3, -4, ….
fn untied(n: usize) -> Vec<(f64, f64)> {
    (1..=n)
        .map(|i| {
            let d = i as f64 * if i % 2 == 0 { -1.0 } else { 1.0 };
            (100.0 + d, 100.0)
        })
        .collect()
}

#[test]
fn the_capability_is_stats_analyze_related_ranks_version_1_engine_1() {
    assert_eq!(CAPABILITY_ID, "stats.analyze.related_ranks");
    assert_eq!((CAPABILITY_VERSION, ENGINE_VERSION), (1, 1));
    assert_eq!((MAX_RELATED_COLUMNS, EXACT_BELOW, ROWS_NOTE_BELOW), (50, 50, 5));
    assert_eq!(check_points(), 1);
}

#[test]
fn every_code_is_a_related_ranks_word_and_holds_no_verdict_word() {
    assert_eq!(C::ALL.len(), 34);
    for c in C::ALL {
        let w = c.wire();
        assert!(w.starts_with("stats.related_ranks."), "{w}");
        let words: Vec<&str> = w["stats.related_ranks.".len()..].split('_').collect();
        for bad in ["should", "recommend", "valid", "accurate"] {
            assert!(!words.contains(&bad), "{w} holds {bad}");
        }
        assert!(!w.contains("significan"), "{w}");
    }
}

#[test]
fn a_missing_code_is_refused_by_index_when_not_finite() {
    let d = numbers(&untied(6));
    let e = refusal(&d, params(&[1, 2], &[f64::NAN]));
    assert_eq!((e.code, int(&e, ParamKey::Index)), (C::MissingCodeNotFinite, Some(0)));
    let e = refusal(&d, params(&[1, 2], &[1.0, f64::INFINITY]));
    assert_eq!((e.code, int(&e, ParamKey::Index)), (C::MissingCodeNotFinite, Some(1)));
    assert!(compute(&d, &params(&[1, 2], &[9.0, 1.0, 9.0])).is_ok(), "any order, repeated or not");
}

#[test]
fn the_list_takes_two_to_fifty_columns_each_once_in_the_users_order() {
    let d = numbers(&untied(6));
    let e = refusal(&d, params(&[1], &[]));
    assert_eq!((e.code, int(&e, ParamKey::Count)), (C::TooFewColumns, Some(1)));
    let e = refusal(&d, params(&[], &[]));
    assert_eq!((e.code, int(&e, ParamKey::Count)), (C::TooFewColumns, Some(0)));
    let wide: Vec<Vec<String>> = (0..51).map(|j| (0..4).map(|r| format!("{}", r * 7 + j)).collect()).collect();
    let d51 = table(&wide);
    let all: Vec<u32> = (1..=51).collect();
    let e = refusal(&d51, params(&all, &[]));
    assert_eq!((e.code, int(&e, ParamKey::Count), int(&e, ParamKey::Limit)), (C::TooManyColumns, Some(51), Some(50)));
    assert!(compute(&d51, &params(&all[..50], &[])).is_ok(), "fifty are taken");
    let d3 = table(&[vec!["1".into(), "2".into(), "3".into()], vec!["4".into(), "5".into(), "6".into()], vec!["7".into(), "8".into(), "9".into()]]);
    // Repeated, and not next to each other.
    let e = refusal(&d3, params(&[2, 1, 2], &[]));
    assert_eq!((e.code, int(&e, ParamKey::Column)), (C::ColumnRepeated, Some(2)));
}

#[test]
fn the_column_refusals_name_the_column_by_alias_in_the_lists_order() {
    let d = pairs(&[("1", "2"), ("x", "3"), ("3", "4"), ("words", "5"), ("5", "6")]);
    let e = refusal(&d, two());
    assert_eq!(
        (e.code, int(&e, ParamKey::Column), int(&e, ParamKey::Count)),
        (C::ColumnHoldsText, Some(1), Some(2))
    );
    let e = refusal(&d, params(&[2, 7], &[]));
    assert_eq!((e.code, int(&e, ParamKey::Column)), (C::UnknownColumn, Some(7)));
    let d = pairs(&[("1", "2"), ("-inf", "3"), ("3", "4"), ("x", "5"), ("5", "6")]);
    let e = refusal(&d, two());
    assert_eq!(
        (e.code, int(&e, ParamKey::Column), int(&e, ParamKey::Count)),
        (C::ColumnNonFinite, Some(1), Some(1))
    );
    let report = validate(&d, &spec(OperationId(1), &d, params(&[1, 2, 2], &[f64::NAN])));
    let codes: Vec<C> = report.errors.iter().map(|e| e.code).collect();
    assert_eq!(codes[..3], [C::MissingCodeNotFinite, C::ColumnRepeated, C::ColumnNonFinite], "{codes:?}");
    for p in e.params.values() {
        assert!(matches!(p, ParamValue::Int(_)), "a count or an alias: {p:?}");
    }
}

#[test]
fn no_complete_row_is_refused_only_when_nothing_else_is() {
    let d = pairs(&[("1", ""), ("", "2"), ("3", "")]);
    let e = refusal(&d, two());
    assert_eq!(e.code, C::NoCompleteRows);
    assert!(e.params.is_empty());
    let report = validate(&d, &spec(OperationId(1), &d, params(&[1, 2], &[f64::NAN])));
    assert_eq!(report.errors.iter().map(|e| e.code).collect::<Vec<C>>(), [C::MissingCodeNotFinite]);
    // A table of no rows.
    let empty = table(&[vec![], vec![]]);
    assert_eq!(refusal(&empty, two()).code, C::NoCompleteRows);
}

#[test]
fn the_test_is_chosen_by_the_count_of_columns() {
    let d = table(&[
        vec!["1".into(), "2".into(), "3".into(), "9".into()],
        vec!["4".into(), "1".into(), "2".into(), "5".into()],
        vec!["7".into(), "8".into(), "1".into(), "2".into()],
    ]);
    assert!(matches!(compute(&d, &params(&[1, 3], &[])).unwrap().test, RelatedTest::Wilcoxon(_)));
    let r = compute(&d, &params(&[3, 1, 2], &[])).unwrap();
    assert_eq!(friedman(&r).df, 2.0);
    assert_eq!(friedman(&r).columns.iter().map(|c| c.column).collect::<Vec<_>>(), [alias(3), alias(1), alias(2)]);
}

#[test]
fn the_differences_are_first_minus_second_and_the_other_order_reverses_the_signs() {
    // Every d of (1, 2) is positive except the smallest.
    let d = numbers(&[(5.0, 1.0), (9.0, 2.0), (3.0, 3.5), (8.0, 2.0), (10.0, 1.0)]);
    let forward = wilcoxon(&compute(&d, &params(&[1, 2], &[])).unwrap());
    assert_eq!((forward.first, forward.second), (alias(1), alias(2)));
    assert_eq!((forward.positive.count, forward.negative.count), (4, 1));
    assert_eq!((forward.v, forward.t_minus), (14.0, 1.0));
    assert!(matches!(forward.z, Stat::Value(z) if z > 0.0));
    assert!(matches!(forward.rank_biserial, Stat::Value(r) if r > 0.0));
    let back = wilcoxon(&compute(&d, &params(&[2, 1], &[])).unwrap());
    assert_eq!((back.first, back.second), (alias(2), alias(1)));
    assert_eq!((back.positive.count, back.negative.count), (1, 4));
    assert_eq!((back.v, back.t_minus), (1.0, 14.0));
    let (Stat::Value(z1), Stat::Value(z2)) = (forward.z, back.z) else { panic!("z") };
    assert_eq!(z1, -z2);
    assert_eq!(forward.p, back.p);
    assert_eq!(forward.exact_p, back.exact_p);
}

#[test]
fn the_exact_p_is_given_without_ties_or_zeros_below_fifty_and_absent_otherwise() {
    let r = compute(&numbers(&untied(49)), &two()).unwrap();
    assert!(wilcoxon(&r).exact_p.is_some());
    assert!(r.notes.is_empty(), "{:?}", r.notes);
    // 50 non-zero pairs: by size.
    let r = compute(&numbers(&untied(50)), &two()).unwrap();
    assert_eq!(wilcoxon(&r).exact_p, None);
    assert_eq!(r.notes, [C::ExactNotGivenSize]);
    // A tied |d|: by the tie.
    let mut rows = untied(6);
    rows.push((102.0, 100.0));
    let r = compute(&numbers(&rows), &two()).unwrap();
    assert_eq!(wilcoxon(&r).exact_p, None);
    assert_eq!(r.notes, [C::ExactNotGivenTies]);
    assert_eq!(wilcoxon(&r).tie_runs, 1);
    // A zero difference: by the zero, and the note that it was dropped.
    let mut rows = untied(6);
    rows.push((7.0, 7.0));
    let r = compute(&numbers(&rows), &two()).unwrap();
    let t = wilcoxon(&r);
    assert_eq!(t.exact_p, None);
    assert_eq!(r.notes, [C::ExactNotGivenZeros, C::ZeroDifferencesDropped]);
    assert_eq!((t.n, t.zero_differences), (6, 1));
    assert_eq!(r.rows_used, 7, "zero pairs count in the rows used");
    // All three, in their order, with a tie and a zero among 50 non-zero pairs.
    let mut rows = untied(50);
    rows[1] = (101.0, 100.0);
    rows.push((7.0, 7.0));
    let r = compute(&numbers(&rows), &two()).unwrap();
    assert_eq!(r.notes, [C::ExactNotGivenTies, C::ExactNotGivenZeros, C::ExactNotGivenSize, C::ZeroDifferencesDropped]);
    // The Friedman test: never an exact p, never its notes.
    let d = table(&[
        (0..6).map(|i| format!("{i}")).collect(),
        (0..6).map(|i| format!("{}", i + 2)).collect(),
        (0..6).map(|i| format!("{}", 10 - i)).collect(),
    ]);
    let r = compute(&d, &params(&[1, 2, 3], &[])).unwrap();
    assert!(r.notes.is_empty(), "{:?}", r.notes);
}

#[test]
fn the_exact_p_is_capped_at_one_where_twice_a_tail_is_above_it() {
    // d = 1, 2, -3: V = 3 is the middle of 0 to 6, P(V <= 3) = P(V >= 3) = 5/8, and
    // twice that is 5/4, so only the cap makes the p 1.
    let r = compute(&numbers(&[(2.0, 1.0), (3.0, 1.0), (0.0, 3.0)]), &two()).unwrap();
    let t = wilcoxon(&r);
    assert_eq!(t.v, 3.0);
    assert_eq!(t.exact_p, Some(1.0));
    // One pair: V = 1 of 1, each tail one half or more, so the p is 1.
    let r = compute(&numbers(&[(3.0, 1.5)]), &two()).unwrap();
    assert_eq!(wilcoxon(&r).exact_p, Some(1.0));
}

/// The independent review's finding: 1 − ties / (n (k³ − k)) cancels when nearly
/// every row is tied, about 1e-11 relative here, where the correction computed
/// from the exact integer difference does not. 99,999 rows of equal values and
/// one row 1, 2, 3: every D_j is -1, 0 or 1, C is 12 / 1,200,000 and Q is exactly 2.
#[test]
fn nearly_every_row_tied_keeps_q_at_its_exact_value() {
    let mut columns = vec![vec!["2".to_string(); 99_999]; 3];
    for (c, v) in columns.iter_mut().zip(["1", "2", "3"]) {
        c.push(v.to_string());
    }
    let r = compute(&table(&columns), &params(&[1, 2, 3], &[])).unwrap();
    let f = friedman(&r);
    let Stat::Value(q) = f.q else { panic!("{:?}", f.q) };
    assert!((q - 2.0).abs() / 2.0 <= 1e-15, "Q = {q:e}");
}

#[test]
fn every_difference_zero_leaves_z_p_and_the_correlation_not_computable() {
    let r = compute(&numbers(&[(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]), &two()).unwrap();
    let t = wilcoxon(&r);
    let nc = Stat::NotComputable(C::AllDifferencesZero);
    assert_eq!((t.z, t.p, t.rank_biserial), (nc, nc, nc));
    assert_eq!((t.n, t.zero_differences, t.v, t.t_minus), (0, 3, 0.0, 0.0));
    assert_eq!((t.negative.mean_rank, t.positive.mean_rank), (None, None));
    assert_eq!(t.median_difference, Stat::Value(0.0));
    assert_eq!(t.exact_p, None);
    assert_eq!(r.notes, [C::ExactNotGivenZeros, C::ZeroDifferencesDropped, C::RowsBelowFive]);
}

#[test]
fn every_row_tied_leaves_q_p_and_w_not_computable() {
    let c: Vec<String> = ["1", "2", "3", "4"].iter().map(|s| s.to_string()).collect();
    let r = compute(&table(&[c.clone(), c.clone(), c]), &params(&[1, 2, 3], &[])).unwrap();
    let t = friedman(&r);
    let nc = Stat::NotComputable(C::AllValuesTied);
    assert_eq!((t.q, t.p, t.kendalls_w), (nc, nc, nc));
    assert_eq!(t.df, 2.0);
    assert_eq!(t.columns.iter().map(|c| c.mean_rank).collect::<Vec<_>>(), [2.0, 2.0, 2.0]);
    assert_eq!(r.notes, [C::RowsBelowFive]);
}

#[test]
fn a_side_with_no_difference_has_count_and_sum_zero_and_no_mean() {
    let r = compute(&numbers(&[(5.0, 1.0), (9.0, 2.0), (8.0, 3.5), (9.5, 2.0), (10.0, 1.0)]), &two()).unwrap();
    let t = wilcoxon(&r);
    assert_eq!((t.negative.count, t.negative.rank_sum, t.negative.mean_rank), (0, 0.0, None));
    assert_eq!(t.positive.count, 5);
    assert_eq!(t.positive.mean_rank, Some(3.0));
    let v: serde_json::Value = serde_json::from_slice(&r.canonical_bytes()).unwrap();
    assert!(v["test"]["wilcoxon"]["negative"]["meanRank"].is_null(), "null, never 0 / 0");
}

#[test]
fn a_row_is_left_out_once_under_the_first_column_in_the_lists_order_without_a_number() {
    let d = table(&[
        vec!["1".into(), "".into(), "3".into(), "-9".into(), "5".into(), "6".into()],
        vec!["2".into(), "4".into(), "".into(), "".into(), "-9".into(), "8".into()],
        vec!["3".into(), "1".into(), "2".into(), "9".into(), "".into(), "1".into()],
    ]);
    let r = compute(&d, &params(&[1, 2, 3], &[-9.0])).unwrap();
    let got: Vec<(C, u64, Option<&ParamValue<C>>)> =
        r.exclusions.iter().map(|e| (e.code, e.count, e.params.get(&ParamKey::Column))).collect();
    assert_eq!(
        got,
        [
            (C::MissingValue, 1, Some(&ParamValue::Int(1))),
            (C::MissingCode, 1, Some(&ParamValue::Int(1))),
            (C::MissingValue, 1, Some(&ParamValue::Int(2))),
            (C::MissingCode, 1, Some(&ParamValue::Int(2))),
        ]
    );
    assert_eq!(r.rows_used, 2);
    assert_eq!(r.rows_used + got.iter().map(|g| g.1).sum::<u64>(), 6, "every row once");
    // In another order, the same rows go under another first column.
    let r = compute(&d, &params(&[3, 2, 1], &[-9.0])).unwrap();
    let got: Vec<(C, u64, Option<&ParamValue<C>>)> =
        r.exclusions.iter().map(|e| (e.code, e.count, e.params.get(&ParamKey::Column))).collect();
    assert_eq!(
        got,
        [
            (C::MissingValue, 1, Some(&ParamValue::Int(3))),
            (C::MissingValue, 2, Some(&ParamValue::Int(2))),
            (C::MissingValue, 1, Some(&ParamValue::Int(1))),
        ]
    );
}

#[test]
fn a_code_applies_to_every_chosen_column_and_minus_zero_leaves_out_zero() {
    let d = pairs(&[("1", "99"), ("99", "2"), ("3", "4"), ("0", "5"), ("-0", "6"), ("7", "8.5")]);
    let r = compute(&d, &params(&[1, 2], &[99.0])).unwrap();
    assert_eq!(r.rows_used, 4);
    let r = compute(&d, &params(&[1, 2], &[-0.0])).unwrap();
    assert_eq!(r.exclusions[0].count, 2, "0 and -0 both left out");
    assert_eq!(r.rows_used, 4);
    // -0 and 0 differ by zero: a zero pair.
    let d = pairs(&[("-0", "0"), ("1", "2"), ("3", "5.5")]);
    assert_eq!(wilcoxon(&compute(&d, &two()).unwrap()).zero_differences, 1);
}

#[test]
fn two_finite_values_whose_difference_overflows_are_ranked_by_size_and_nothing_is_refused() {
    let big = 1.5 * 2f64.powi(1023);
    let r = compute(&numbers(&[(big, -big), (1.0, 5.0), (2.0, 7.5), (3.0, 12.25), (10.0, 2.0)]), &two()).unwrap();
    let t = wilcoxon(&r);
    // The overflowing pair is the largest in size: rank 5, positive.
    assert_eq!(t.v, 8.0);
    assert!(matches!(t.z, Stat::Value(_)));
    assert_eq!(t.median_difference, Stat::Value(-4.0));
    // Two of opposite infinite size: their median is not finite.
    let r = compute(&numbers(&[(big, -big), (-big, big)]), &two()).unwrap();
    assert_eq!(wilcoxon(&r).median_difference, Stat::NotComputable(C::Overflow));
    assert_eq!(wilcoxon(&r).tie_runs, 1, "two infinities tie, as R's ranks have them");
}

/// Values near both ends of the doubles, whose differences all stay finite (decision
/// 652): the overflow's own test is
/// `two_finite_values_whose_difference_overflows_are_ranked_by_size_and_nothing_is_refused`.
#[test]
fn every_number_is_finite_at_both_ends_of_the_doubles() {
    let d = numbers(&[(1.7976931348623157e308, 1.5e308), (1e308, 1.7976931348623157e308), (5e-324, 1e-320), (1e-300, 2e-300), (-1e308, -1.5e308)]);
    let r = compute(&d, &two()).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.canonical_bytes()).unwrap();
    fn no_null(v: &serde_json::Value, path: &str) {
        match v {
            serde_json::Value::Null => assert!(path.ends_with("exactP") || path.ends_with("meanRank"), "a null at {path}"),
            serde_json::Value::Number(n) => assert!(n.as_f64().is_some_and(f64::is_finite), "{path}: {n}"),
            serde_json::Value::Array(a) => a.iter().for_each(|x| no_null(x, path)),
            serde_json::Value::Object(o) => o.iter().for_each(|(k, x)| no_null(x, &format!("{path}.{k}"))),
            _ => {}
        }
    }
    no_null(&v, "");
    assert!(r.columns.iter().all(|c| c.median.is_finite()));
}

#[test]
fn fewer_than_five_pairs_or_rows_are_noted_and_five_are_not() {
    let r = compute(&numbers(&untied(4)), &two()).unwrap();
    assert_eq!(r.notes, [C::RowsBelowFive]);
    assert!(compute(&numbers(&untied(5)), &two()).unwrap().notes.is_empty());
    let c = |f: i32| (0..4).map(|i| format!("{}", (i * f) % 7)).collect::<Vec<String>>();
    assert_eq!(compute(&table(&[c(1), c(2), c(3)]), &params(&[1, 2, 3], &[])).unwrap().notes, [C::RowsBelowFive]);
    let c = |f: i32| (0..5).map(|i| format!("{}", (i * f) % 7)).collect::<Vec<String>>();
    assert!(compute(&table(&[c(1), c(2), c(3)]), &params(&[1, 2, 3], &[])).unwrap().notes.is_empty());
}

#[test]
fn the_rows_order_changes_no_bit() {
    let mut rows = untied(9);
    rows.push((4.5, 4.5));
    let forward = compute(&numbers(&rows), &two()).unwrap();
    rows.reverse();
    let backward = compute(&numbers(&rows), &two()).unwrap();
    assert_eq!(forward.canonical_bytes(), backward.canonical_bytes());
}

#[test]
fn the_run_stops_at_its_one_check_point_before_the_arithmetic() {
    let d = numbers(&untied(6));
    let mut seen = Vec::new();
    let out = run(&d, &two(), |i| {
        seen.push(i);
        ControlFlow::Break("stop")
    });
    assert_eq!(out, Err(Halt::Stopped { reason: "stop" }));
    assert_eq!(seen, [0]);
    let mut seen = Vec::new();
    assert!(run(&d, &two(), |i| {
        seen.push(i);
        ControlFlow::<()>::Continue(())
    })
    .is_ok());
    assert_eq!(seen.len(), check_points());
    let mut reached = false;
    let out = run(&d, &params(&[1, 2], &[f64::NAN]), |_| {
        reached = true;
        ControlFlow::<()>::Continue(())
    });
    assert!(matches!(out, Err(Halt::Refused(_))) && !reached);
}

#[test]
fn the_result_has_exactly_its_keys() {
    let keys = |v: &serde_json::Value| -> BTreeSet<String> { v.as_object().unwrap().keys().cloned().collect() };
    let want = |k: &[&str]| -> BTreeSet<String> { k.iter().map(|s| s.to_string()).collect() };
    let top = want(&["columns", "rowsUsed", "test", "exclusions", "notes", "limitations"]);
    let r = compute(&numbers(&untied(6)), &two()).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.canonical_bytes()).unwrap();
    assert_eq!(keys(&v), top);
    assert_eq!(keys(&v["columns"][0]), want(&["column", "median"]));
    assert_eq!(v["columns"][0]["column"], "C1");
    assert_eq!(keys(&v["test"]), want(&["wilcoxon"]));
    assert_eq!(
        keys(&v["test"]["wilcoxon"]),
        want(&["first", "second", "n", "zeroDifferences", "tieRuns", "negative", "positive", "v", "tMinus", "z", "p", "exactP", "rankBiserial", "medianDifference"])
    );
    assert_eq!(keys(&v["test"]["wilcoxon"]["negative"]), want(&["count", "rankSum", "meanRank"]));
    let c = |f: i32| (0..6).map(|i| format!("{}", (i * f) % 7)).collect::<Vec<String>>();
    let r = compute(&table(&[c(1), c(2), c(3)]), &params(&[1, 2, 3], &[])).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.canonical_bytes()).unwrap();
    assert_eq!(keys(&v), top);
    assert_eq!(keys(&v["test"]["friedman"]), want(&["columns", "q", "df", "p", "kendallsW"]));
    assert_eq!(keys(&v["test"]["friedman"]["columns"][0]), want(&["column", "rankSum", "meanRank"]));
    assert_eq!(
        r.limitations,
        [
            C::NoPairwiseComparisons,
            C::TwoSidedOnly,
            C::NoLocationEstimate,
            C::ExactWithoutTiesOrZerosOnly,
            C::PrattNotOffered,
            C::NoFurtherRelatedTests,
            C::NoWeights,
            C::ColumnOrder,
        ]
    );
}

#[test]
fn validate_warns_of_suspect_codes_in_each_column_once_and_refuses_other_specs() {
    let d = pairs(&[("1", "1"), ("2", "99"), ("3", "2"), ("99", "99"), ("99", "3"), ("4", "5")]);
    let report = validate(&d, &spec(OperationId(3), &d, two()));
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let warned: Vec<(C, Option<i64>, Option<i64>, Option<i64>)> = report
        .warnings
        .iter()
        .map(|w| (w.code, int(w, ParamKey::Column), int(w, ParamKey::Value), int(w, ParamKey::Count)))
        .collect();
    assert_eq!(
        warned,
        [
            (C::SuspectMissingCode, Some(1), Some(99), Some(2)),
            (C::SuspectMissingCode, Some(2), Some(99), Some(2)),
        ]
    );
    let report = validate(&d, &spec(OperationId(3), &d, params(&[1, 2], &[99.0])));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let mut other = spec(OperationId(4), &d, two());
    other.capability_id = CapabilityId::parse("stats.analyze.one_way_anova").unwrap();
    other.capability_version = 2;
    other.workspace_id = WorkspaceId::Compute;
    let report = validate(&d, &other);
    assert_eq!(report.status, ValidationStatus::Unsupported);
    assert_eq!(report.errors[..2].iter().map(|u| u.code).collect::<Vec<C>>(), [C::UnknownCapability, C::CapabilityVersionMismatch]);
    let elsewhere = numbers(&untied(6));
    let report = validate(&d, &spec(OperationId(5), &elsewhere, two()));
    assert_eq!(report.errors[0].code, C::InputMismatch);
    let mut none = spec(OperationId(6), &d, two());
    none.inputs.clear();
    let report = validate(&d, &none);
    assert_eq!((report.errors[0].code, int(&report.errors[0], ParamKey::Count)), (C::InputCount, Some(0)));
}

#[test]
fn the_spec_is_camel_case_and_refuses_unknown_fields() {
    let p: RelatedRanksParams = serde_json::from_str(r#"{"columns":["C2","C1"],"missingCodes":[-9]}"#).unwrap();
    assert_eq!(p, params(&[2, 1], &[-9.0]));
    assert!(serde_json::from_str::<RelatedRanksParams>(r#"{"columns":["C1","C2"],"missingCodes":[],"levels":null}"#).is_err());
    assert!(serde_json::from_str::<RelatedRanksParams>(r#"{"columns":["C1","C2"]}"#).is_err(), "missingCodes is required");
}
