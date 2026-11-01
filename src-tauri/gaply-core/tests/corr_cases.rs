//! **`gaply_core::stats_corr`'s rules beyond the R golden (Research OS,
//! Phase 8H, decisions 338 and 342):** one choice, one spec (a variable,
//! coefficient or code repeated or out of order is refused, never sorted);
//! text and non-finite columns refused; the rows adding up under both
//! handlings; the not-computable cases decided from the values; the check
//! points, one before the arithmetic and one before each pair of each
//! coefficient; the budget; and a result with no null and no infinity.
//! Synthetic data only.

use std::ops::ControlFlow;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ParamKey, ParamValue};
use gaply_core::stats_corr::{
    check_points, compute, estimated_seconds, run, spec, validate, Bounded, Coefficient, CorrCode,
    CorrParams, Halt, Interval, MissingHandling, PairOutcome, TestStatistic, BUDGET_SECONDS,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

const ALL: [Coefficient; 3] = [
    Coefficient::Pearson,
    Coefficient::Spearman,
    Coefficient::Kendall,
];

fn dataset(headers: &[&str], rows: &[&[&str]]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter()
                .map(|r| r.iter().map(|c| c.to_string()).collect())
                .collect(),
        ),
        DataFingerprint::of_bytes(
            format!("{headers:?}{rows:?}").as_bytes(),
            "corr cases",
            TableRef::FirstTable,
        ),
    )
    .expect("dataset")
}

fn c(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn params(vars: &[u32], coefficients: &[Coefficient], missing: MissingHandling) -> CorrParams {
    CorrParams {
        variables: vars.iter().map(|v| c(*v)).collect(),
        coefficients: coefficients.to_vec(),
        missing,
        missing_codes: Vec::new(),
    }
}

/// Four numeric columns of eight rows, some cells empty, one -9.
fn four() -> Dataset {
    dataset(
        &["a", "b", "c", "d"],
        &[
            &["1", "2.5", "3", "8"],
            &["2", "", "1", "7"],
            &["3", "4", "", "-9"],
            &["4", "3.5", "6", "5"],
            &["", "6", "5", "4"],
            &["6", "7.5", "8", "3"],
            &["7", "", "", "2"],
            &["8", "9", "7", "1"],
        ],
    )
}

fn refused_code(d: &Dataset, p: &CorrParams) -> String {
    compute(d, p).expect_err("refused").code.wire().to_string()
}

fn error_codes(d: &Dataset, p: &CorrParams) -> Vec<String> {
    let s = spec(OperationId(1), d, p.clone());
    validate(d, &s)
        .errors
        .iter()
        .map(|e| e.code.wire().to_string())
        .collect()
}

#[test]
fn variables_coefficients_and_codes_repeated_or_out_of_order_are_refused_never_sorted() {
    let d = four();
    let p = params(&[2, 1], &ALL, MissingHandling::Pairwise);
    assert_eq!(refused_code(&d, &p), "stats.corr.variables_out_of_order");
    let p = params(&[1, 1, 2], &ALL, MissingHandling::Pairwise);
    assert_eq!(refused_code(&d, &p), "stats.corr.variable_repeated");
    let p = params(
        &[1, 2],
        &[Coefficient::Kendall, Coefficient::Pearson],
        MissingHandling::Pairwise,
    );
    assert_eq!(refused_code(&d, &p), "stats.corr.coefficients_out_of_order");
    let p = params(
        &[1, 2],
        &[Coefficient::Pearson, Coefficient::Pearson],
        MissingHandling::Pairwise,
    );
    assert_eq!(refused_code(&d, &p), "stats.corr.coefficient_repeated");
    let p = params(&[1, 2], &[], MissingHandling::Pairwise);
    assert_eq!(refused_code(&d, &p), "stats.corr.no_coefficient");
    let p = params(&[1], &ALL, MissingHandling::Pairwise);
    assert_eq!(refused_code(&d, &p), "stats.corr.too_few_variables");
    let mut p = params(&[1, 2], &ALL, MissingHandling::Pairwise);
    p.missing_codes = vec![9.0, -9.0];
    assert_eq!(
        refused_code(&d, &p),
        "stats.corr.missing_codes_out_of_order"
    );
    p.missing_codes = vec![-9.0, -9.0];
    assert_eq!(refused_code(&d, &p), "stats.corr.missing_code_repeated");
    p.missing_codes = vec![f64::NAN];
    assert_eq!(refused_code(&d, &p), "stats.corr.missing_code_not_finite");
    // In order, it runs; validate says the same as run.
    let p = params(&[1, 2, 4], &ALL, MissingHandling::Pairwise);
    assert!(compute(&d, &p).is_ok());
    assert!(error_codes(&d, &p).is_empty());
    assert_eq!(
        error_codes(&d, &params(&[4, 2], &ALL, MissingHandling::Pairwise)),
        ["stats.corr.variables_out_of_order"]
    );
}

#[test]
fn a_column_of_text_or_an_unknown_one_is_refused() {
    let d = dataset(
        &["x", "t", "y"],
        &[
            &["1", "a", "2"],
            &["2", "b", "1"],
            &["3", "c", "5"],
            &["4", "d", "3"],
        ],
    );
    let e = compute(&d, &params(&[1, 2, 3], &ALL, MissingHandling::Pairwise)).expect_err("text");
    assert_eq!(e.code, CorrCode::VariableHoldsText);
    assert_eq!(e.params.get(&ParamKey::Column), Some(&ParamValue::Int(2)));
    assert_eq!(e.params.get(&ParamKey::Count), Some(&ParamValue::Int(4)));
    let e = compute(&d, &params(&[1, 9], &ALL, MissingHandling::Pairwise)).expect_err("unknown");
    assert_eq!(e.code, CorrCode::UnknownColumn);
}

#[test]
fn the_rows_used_and_the_exclusions_add_up_to_the_rows_read_under_both_handlings() {
    let d = four();
    let mut p = params(&[1, 2, 3, 4], &ALL, MissingHandling::Listwise);
    p.missing_codes = vec![-9.0];
    let r = compute(&d, &p).unwrap();
    let excluded: u64 = r.exclusions.iter().map(|e| e.count).sum();
    assert_eq!(r.rows_used + excluded, 8);
    assert_eq!(r.rows_used, 4, "rows 1, 4, 6 and 8 are complete");
    // Each left-out row under its first variable without a value: row 2 (b
    // empty), row 3 (c empty before d's code), row 5 (a empty), row 7 (b).
    let got: Vec<(String, i64, u64)> = r
        .exclusions
        .iter()
        .map(|e| {
            let Some(&ParamValue::Int(col)) = e.params.get(&ParamKey::Column) else {
                panic!("a listwise exclusion names its column")
            };
            (e.code.wire().to_string(), col, e.count)
        })
        .collect();
    assert_eq!(
        got,
        [
            ("stats.corr.missing_value".to_string(), 1, 1),
            ("stats.corr.missing_value".to_string(), 2, 2),
            ("stats.corr.missing_value".to_string(), 3, 1),
        ]
    );
    // Pairwise: a row is used with values in two variables; row 7 has a and
    // d, so it is used; none is left out.
    p.missing = MissingHandling::Pairwise;
    let r = compute(&d, &p).unwrap();
    assert_eq!(r.rows_used, 8);
    assert!(r.exclusions.is_empty());
    // Two variables, a row with one value is left out, naming no column.
    let r = compute(&d, &params(&[2, 3], &ALL, MissingHandling::Pairwise)).unwrap();
    assert_eq!(r.rows_used, 5);
    assert_eq!(r.exclusions.len(), 1);
    assert_eq!(r.exclusions[0].code, CorrCode::FewerThanTwoValues);
    assert_eq!(r.exclusions[0].count, 3);
    assert!(r.exclusions[0].params.is_empty());
}

#[test]
fn listwise_with_fewer_than_3_complete_rows_and_pairwise_with_no_pair_of_3_are_refused() {
    let d = dataset(
        &["x", "y", "z"],
        &[
            &["1", "2", ""],
            &["2", "1", ""],
            &["3", "", "4"],
            &["", "5", "6"],
            &["5", "", "1"],
        ],
    );
    assert_eq!(
        refused_code(&d, &params(&[1, 2, 3], &ALL, MissingHandling::Listwise)),
        "stats.corr.too_few_complete_rows"
    );
    let d = dataset(
        &["x", "y"],
        &[&["1", "2"], &["2", "1"], &["3", ""], &["", "5"]],
    );
    assert_eq!(
        refused_code(&d, &params(&[1, 2], &ALL, MissingHandling::Pairwise)),
        "stats.corr.no_pair_of_three"
    );
}

#[test]
fn a_constant_variable_makes_each_of_its_pairs_not_computable_and_leaves_the_rest() {
    let d = dataset(
        &["x", "k", "y"],
        &[
            &["1", "4", "2"],
            &["2", "4", "1"],
            &["3", "4", "4"],
            &["4", "4", "3"],
            &["5", "4", "6"],
        ],
    );
    let r = compute(&d, &params(&[1, 2, 3], &ALL, MissingHandling::Pairwise)).unwrap();
    for table in &r.coefficients {
        for pair in &table.pairs {
            let touches_k = pair.first == c(2) || pair.second == c(2);
            match pair.outcome {
                PairOutcome::NotComputable(code) => {
                    assert!(touches_k, "{pair:?}");
                    assert_eq!(code, CorrCode::VariableConstant);
                }
                PairOutcome::Computed(_) => assert!(!touches_k, "{pair:?}"),
            }
        }
    }
}

#[test]
fn a_perfect_coefficient_holds_its_code_in_place_of_t_and_the_result_has_no_null() {
    let d = dataset(
        &["x", "y", "z"],
        &[
            &["1", "3", "9"],
            &["2", "5", "7"],
            &["3", "7", "6"],
            &["4", "9", "2"],
            &["5", "11", "1"],
        ],
    );
    let r = compute(&d, &params(&[1, 2, 3], &ALL, MissingHandling::Pairwise)).unwrap();
    let mut perfect = 0;
    for table in &r.coefficients {
        for pair in &table.pairs {
            let PairOutcome::Computed(s) = pair.outcome else {
                panic!("{pair:?}")
            };
            if s.estimate.abs() == 1.0 {
                perfect += 1;
                match s.test {
                    TestStatistic::T(t) => {
                        assert_eq!(t.t, Bounded::Infinite(CorrCode::PerfectCoefficient));
                        assert_eq!(s.p, 0.0);
                    }
                    TestStatistic::Z(z) => {
                        assert!(z.is_finite() && s.p > 0.0, "Kendall's p comes from z")
                    }
                }
            }
        }
    }
    // x and y, an exact line: Pearson, Spearman and Kendall +1; x and z, y
    // and z: Spearman and Kendall -1. stats_verify's pearson_r gives 1 - 2
    // ulp on this line, where R's cor gives exactly 1 and an infinite t; a
    // Pearson r within RHO_ONE_ULPS of +-1 is +-1, as Level 1's rho is
    // (decision 352, 8H's open question 1). Before it, this pinned 1 - 2 ulp
    // and a finite t of about 8.2e7.
    assert_eq!(perfect, 7);
    let PairOutcome::Computed(xy) = r.coefficients[0].pairs[0].outcome else {
        panic!()
    };
    assert_eq!(xy.estimate, 1.0);
    let TestStatistic::T(t) = xy.test else {
        panic!()
    };
    assert_eq!(t.t, Bounded::Infinite(CorrCode::PerfectCoefficient));
    assert_eq!((t.df, xy.p), (3.0, 0.0));
    assert_eq!(
        xy.interval,
        Interval::Computed {
            lower: 1.0,
            upper: 1.0
        }
    );
    let text = String::from_utf8(r.canonical_bytes()).unwrap();
    // serde writes a non-finite f64 as null, so no null means no infinity.
    assert!(!text.contains("null") && !text.contains("NaN"), "{text}");
    assert!(
        text.contains(r#"{"infinite":"stats.corr.perfect_coefficient"}"#),
        "{text}"
    );
}

/// The check points: one before the arithmetic, one before each pair of
/// each coefficient, and a stop during the pairs ends the run with no
/// result.
#[test]
fn the_check_points_count_from_the_spec_and_a_stop_during_the_pairs_returns_no_result() {
    let d = four();
    let p = params(&[1, 2, 3, 4], &ALL, MissingHandling::Pairwise);
    assert_eq!(check_points(&p), 1 + 3 * 6);
    let mut seen = Vec::new();
    let r = run(&d, &p, |i| {
        seen.push(i);
        ControlFlow::<()>::Continue(())
    });
    assert!(r.is_ok());
    assert_eq!(seen, (0..check_points(&p)).collect::<Vec<_>>());
    let mut calls = 0;
    let r = run(&d, &p, |i| {
        calls += 1;
        if i == 8 {
            ControlFlow::Break("stop")
        } else {
            ControlFlow::Continue(())
        }
    });
    assert_eq!(r, Err(Halt::Stopped { reason: "stop" }));
    assert_eq!(calls, 9);
}

/// The budget (decision 342): a spec whose estimate passes 15 s is refused
/// before the run, naming the pairs and the rows; one row fewer is not;
/// and the same rows with fewer coefficients or variables pass. The rows
/// are the fewest the module's own rates refuse, so the test follows the
/// rates wherever they live.
#[test]
fn a_spec_over_the_budget_is_refused_with_its_pairs_and_rows() {
    let k = 128u32;
    let all: Vec<u32> = (1..=k).collect();
    let mut p = params(&all, &ALL, MissingHandling::Pairwise);
    let mut rows = 1usize;
    while estimated_seconds(&p, rows) <= BUDGET_SECONDS {
        rows *= 2;
    }
    let (mut lo, mut hi) = (rows / 2, rows);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if estimated_seconds(&p, mid) > BUDGET_SECONDS {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let headers: Vec<String> = (1..=k).map(|i| format!("v{i}")).collect();
    let make = |n: usize| {
        build_dataset(
            RawTable::from_rows(
                headers.clone(),
                (0..n as u32)
                    .map(|r| {
                        (1..=k)
                            .map(|v| ((r * 7 + v * 13) % 97).to_string())
                            .collect()
                    })
                    .collect(),
            ),
            DataFingerprint::of_bytes(
                format!("budget {n}").as_bytes(),
                "corr cases",
                TableRef::FirstTable,
            ),
        )
        .unwrap()
    };
    let over = make(hi);
    let e = compute(&over, &p).expect_err("over the budget");
    assert_eq!(e.code, CorrCode::OverBudget);
    assert_eq!(
        e.params.get(&ParamKey::Count),
        Some(&ParamValue::Int(i64::from(k * (k - 1) / 2)))
    );
    assert_eq!(
        e.params.get(&ParamKey::Found),
        Some(&ParamValue::Int(hi as i64))
    );
    let s = spec(OperationId(1), &over, p.clone());
    assert!(validate(&over, &s)
        .errors
        .iter()
        .any(|e| e.code == CorrCode::OverBudget));
    assert!(estimated_seconds(&p, lo) <= BUDGET_SECONDS);
    // Fewer coefficients, or fewer variables, bring it under.
    p.coefficients = vec![Coefficient::Pearson];
    p.variables.truncate(2);
    assert!(estimated_seconds(&p, hi) <= BUDGET_SECONDS);
}

#[test]
fn a_candidate_missing_code_the_user_did_not_tick_is_a_warning() {
    let d = dataset(
        &["x", "y"],
        &[
            &["1", "2"],
            &["2", "-99"],
            &["3", "4"],
            &["4", "3"],
            &["5", "-99"],
            &["6", "7"],
        ],
    );
    let p = params(&[1, 2], &ALL, MissingHandling::Pairwise);
    let report = validate(&d, &spec(OperationId(1), &d, p.clone()));
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.code == CorrCode::SuspectMissingCode),
        "{:?}",
        report.warnings
    );
    let mut ticked = p;
    ticked.missing_codes = vec![-99.0];
    let report = validate(&d, &spec(OperationId(1), &d, ticked));
    assert!(report.warnings.is_empty());
}
