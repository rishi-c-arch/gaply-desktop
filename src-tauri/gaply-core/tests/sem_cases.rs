//! **`gaply_core::stats_sem`'s cases (Research OS, Phase 9C-4, decisions 797 to
//! 800)**, beside the R golden and the AMOS tier: every check by code and
//! params in decision 797's order, with each family's first-error rule; the
//! identification check's counts on known specs, against R's reference ranks
//! (~/gaply-corpus/os/phase9/9c4/logs/ff_identification.log); the
//! not-computable paths and the notes; permuting the rows and multiplying every
//! value by 1,000; the attempts; the Walsh columns; a CFA spec fitted by both
//! methods; repeated runs, the result's exact keys and the check points; and
//! the widest result's size.

use std::ops::ControlFlow;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ParamKey, ParamValue};
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_sem::{
    check_points, compute, compute_with_caps, run, spec, validate, Halt, SemCode as C, SemOutcome, SemParams, Stat, MAX_COLUMNS, MAX_COVARIANCES,
    MAX_EQUAL_MEMBERS, MAX_FACTORS, MAX_ITERATIONS, MAX_PATHS,
};

const CFA_DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_cfa/data");
const SEM_DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_sem/data");

fn dataset(headers: &[String], rows: &[Vec<String>]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers.to_vec(), rows.to_vec()),
        DataFingerprint::of_bytes(format!("{headers:?}{}{rows:?}", rows.len()).as_bytes(), "sem cases", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn table(columns: &[Vec<f64>]) -> Dataset {
    let headers: Vec<String> = (1..=columns.len()).map(|j| format!("x{j}")).collect();
    let rows: Vec<Vec<String>> = (0..columns[0].len()).map(|r| columns.iter().map(|c| format!("{:?}", c[r])).collect()).collect();
    dataset(&headers, &rows)
}

fn file(path: &str) -> Dataset {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    dataset(&headers, &rows)
}

/// The file's columns as numbers, column by column.
fn columns_of(path: &str) -> Vec<Vec<f64>> {
    let text = std::fs::read_to_string(path).unwrap();
    let rows: Vec<Vec<f64>> = text.lines().filter(|l| !l.starts_with('#')).skip(1).map(|l| l.split(',').map(|x| x.parse().unwrap()).collect()).collect();
    (0..rows[0].len()).map(|j| rows.iter().map(|r| r[j]).collect()).collect()
}

fn sem(json: &str) -> SemParams {
    serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

fn codes(errors: &[gaply_core::contracts::Diagnostic<C>]) -> Vec<&'static str> {
    errors.iter().map(|e| e.code.wire()).collect()
}

fn int(e: &gaply_core::contracts::Diagnostic<C>, k: ParamKey) -> Option<i64> {
    match e.params.get(&k) {
        Some(ParamValue::Int(n)) => Some(*n),
        _ => None,
    }
}

fn val(s: Stat) -> f64 {
    match s {
        Stat::Value(x) => x,
        Stat::NotComputable(c) => panic!("not computable: {}", c.wire()),
    }
}

/// K factors over n rows and p columns, every value an integer (the CFA's
/// cases' `structured`).
fn structured(n: u64, p: u64, k: u64) -> Vec<Vec<f64>> {
    (1..=p)
        .map(|j| {
            (1..=n)
                .map(|i| {
                    let f = (j - 1) % k;
                    let score = ((i * (7907 + 104729 * f)) % 1013) as f64 - 506.0;
                    let shared = ((i * 2654435761) % 1009) as f64 - 504.0;
                    let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as f64 - 498.0;
                    (2.0 + (j % 3) as f64) * (score + shared) + noise
                })
                .collect()
        })
        .collect()
}

fn example_8() -> Dataset {
    file(&format!("{CFA_DATA}/example8.csv"))
}

const EXAMPLE_8: &str = r#"{"indicators":[["C1","C2","C3"],["C4","C5","C6"]],"paths":[],"covariances":[{"a":{"factor":1},"b":{"factor":2}}],"equal":[],"missingCodes":[]}"#;
const WHEATON_A: &str = r#"{"indicators":[["C1","C2"],["C3","C4"],["C5","C6"]],"paths":[{"to":{"factor":1},"from":{"factor":3}},{"to":{"factor":2},"from":{"factor":1}},{"to":{"factor":2},"from":{"factor":3}}],"covariances":[],"equal":[],"missingCodes":[]}"#;
const EXAMPLE_7: &str = r#"{"indicators":[],"paths":[{"to":{"column":"C1"},"from":{"column":"C2"}},{"to":{"column":"C1"},"from":{"column":"C3"}},{"to":{"column":"C2"},"from":{"column":"C1"}},{"to":{"column":"C2"},"from":{"column":"C4"}},{"to":{"column":"C2"},"from":{"column":"C5"}},{"to":{"column":"C2"},"from":{"column":"C6"}}],"covariances":[{"a":{"column":"C1"},"b":{"column":"C2"}},{"a":{"column":"C3"},"b":{"column":"C4"}},{"a":{"column":"C3"},"b":{"column":"C5"}},{"a":{"column":"C3"},"b":{"column":"C6"}},{"a":{"column":"C4"},"b":{"column":"C5"}},{"a":{"column":"C4"},"b":{"column":"C6"}},{"a":{"column":"C5"},"b":{"column":"C6"}}],"equal":[],"missingCodes":[]}"#;

/* ---- the checks (decision 797) --------------------------------------------- */

#[test]
fn the_spec_checks_come_in_decision_797s_order_and_validate_lists_every_one() {
    let mut cols = structured(40, 8, 2);
    cols[1][3] = f64::INFINITY;
    let mut rows: Vec<Vec<String>> = (0..40).map(|r| cols.iter().map(|c| format!("{:?}", c[r])).collect()).collect();
    rows[3][2] = "abc".into();
    let headers: Vec<String> = (1..=8).map(|j| format!("x{j}")).collect();
    let d = dataset(&headers, &rows);
    // One error of each family, in the order the checks run: the codes; a factor
    // of one column, columns out of order, factors out of order, a column twice;
    // a path naming factor 9, a path to itself, a path at a factor's column, the
    // paths out of order, a path twice; a covariance naming factor 9, one with
    // itself, out of order, twice; a group of one, a group of two kinds, an
    // unknown member, a first loading, a group out of order, a member twice; text,
    // a value not finite, an unknown column.
    let p = sem(
        r#"{"indicators":[["C4","C2"],["C1"],["C3","C4","C9"]],
            "paths":[{"to":{"factor":9},"from":{"column":"C5"}},{"to":{"column":"C6"},"from":{"column":"C6"}},{"to":{"column":"C7"},"from":{"column":"C3"}},
                     {"to":{"column":"C7"},"from":{"column":"C5"}},{"to":{"column":"C7"},"from":{"column":"C5"}},{"to":{"column":"C6"},"from":{"column":"C5"}}],
            "covariances":[{"a":{"factor":9},"b":{"column":"C5"}},{"a":{"column":"C5"},"b":{"column":"C5"}},{"a":{"column":"C7"},"b":{"column":"C5"}},{"a":{"column":"C7"},"b":{"column":"C5"}}],
            "equal":[[{"variance":{"variable":{"column":"C5"}}}],
                     [{"path":{"to":{"column":"C7"},"from":{"column":"C5"}}},{"variance":{"variable":{"column":"C7"}}}],
                     [{"loading":{"factor":1,"column":"C4"}},{"loading":{"factor":3,"column":"C8"}}],
                     [{"variance":{"variable":{"column":"C6"}}},{"variance":{"variable":{"column":"C5"}}},{"variance":{"variable":{"column":"C6"}}}]],
            "missingCodes":[]}"#,
    );
    let mut p = p;
    p.missing_codes = vec![f64::NAN];
    let r = validate(&d, &spec(OperationId(1), &d, p));
    let got = codes(&r.errors);
    let want = [
        C::MissingCodeNotFinite,
        C::FactorTooFewItems,
        C::ItemsOutOfOrder,
        C::FactorsOutOfOrder,
        C::ItemRepeated,
        C::UnknownFactor,
        C::PathToItself,
        C::IndicatorInPath,
        C::PathRepeated,
        C::PathsOutOfOrder,
        C::UnknownFactor,
        C::CovarianceWithItself,
        C::CovariancesOutOfOrder,
        C::CovariancesOutOfOrder,
        C::CovarianceRepeated,
        // Group 1, a variance alone; group 2 mixed, and its first member (a path) before
        // group 1's (a variance); group 3 a first loading and a column factor 3 does not
        // have, and before group 2; group 4 out of order within, a variance from group 1
        // again and its own first twice.
        C::EqualGroupTooSmall,
        C::EqualMixedKinds,
        C::EqualOutOfOrder,
        C::EqualFixedParameter,
        C::EqualUnknownParameter,
        C::EqualOutOfOrder,
        C::EqualOutOfOrder,
        C::EqualRepeated,
        C::EqualRepeated,
        // The columns in alias order: C2 not finite, C3 text, C9 unknown.
        C::ColumnNonFinite,
        C::ColumnHoldsText,
        C::UnknownColumn,
    ];
    assert_eq!(got, want.iter().map(|c| c.wire()).collect::<Vec<_>>());
    // The run refuses with the first.
    let run_error = compute(&d, &sem(r#"{"indicators":[["C2","C1"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#)).unwrap_err();
    assert_eq!(run_error.code, C::ItemsOutOfOrder);
}

#[test]
fn each_check_names_its_params() {
    let d = table(&structured(40, 8, 2));
    let first = |json: &str| validate(&d, &spec(OperationId(1), &d, sem(json))).errors.into_iter().next().expect("an error");
    let e = first(r#"{"indicators":[],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#);
    assert_eq!(e.code, C::EmptyModel);
    let e = first(r#"{"indicators":[["C1","C2"],["C3"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#);
    assert_eq!((e.code, int(&e, ParamKey::Index), int(&e, ParamKey::Count), int(&e, ParamKey::Limit)), (C::FactorTooFewItems, Some(2), Some(1), Some(2)));
    let e = first(r#"{"indicators":[["C1","C2"]],"paths":[{"to":{"factor":1},"from":{"column":"C3"}},{"to":{"factor":2},"from":{"column":"C3"}}],"covariances":[],"equal":[],"missingCodes":[]}"#);
    assert_eq!((e.code, int(&e, ParamKey::Index)), (C::UnknownFactor, Some(2)));
    let e = first(r#"{"indicators":[["C1","C2"]],"paths":[{"to":{"factor":1},"from":{"column":"C2"}}],"covariances":[],"equal":[],"missingCodes":[]}"#);
    assert_eq!((e.code, int(&e, ParamKey::Column)), (C::IndicatorInPath, Some(2)));
    let e = first(r#"{"indicators":[],"paths":[{"to":{"column":"C2"},"from":{"column":"C1"}},{"to":{"column":"C1"},"from":{"column":"C3"}}],"covariances":[],"equal":[],"missingCodes":[]}"#);
    assert_eq!((e.code, int(&e, ParamKey::Index)), (C::PathsOutOfOrder, Some(2)));
    let e = first(r#"{"indicators":[],"paths":[],"covariances":[{"a":{"column":"C2"},"b":{"column":"C1"}}],"equal":[],"missingCodes":[]}"#);
    assert_eq!((e.code, int(&e, ParamKey::Index)), (C::CovariancesOutOfOrder, Some(1)));
    let e = first(r#"{"indicators":[["C1","C2","C3"]],"paths":[],"covariances":[],"equal":[[{"variance":{"variable":{"factor":1}}},{"loading":{"factor":1,"column":"C3"}}]],"missingCodes":[]}"#);
    assert_eq!((e.code, int(&e, ParamKey::Index)), (C::EqualMixedKinds, Some(1)));
    let e = first(r#"{"indicators":[["C1","C2","C3"]],"paths":[],"covariances":[],"equal":[[{"loading":{"factor":1,"column":"C2"}},{"loading":{"factor":1,"column":"C4"}}]],"missingCodes":[]}"#);
    assert_eq!((e.code, int(&e, ParamKey::Index), int(&e, ParamKey::Value)), (C::EqualUnknownParameter, Some(1), Some(2)));
    let e = first(r#"{"indicators":[["C1","C2","C3"]],"paths":[],"covariances":[],"equal":[[{"loading":{"factor":1,"column":"C1"}},{"loading":{"factor":1,"column":"C2"}}]],"missingCodes":[]}"#);
    assert_eq!((e.code, int(&e, ParamKey::Index), int(&e, ParamKey::Value)), (C::EqualFixedParameter, Some(1), Some(1)));
    let e = first(r#"{"indicators":[["C1","C2","C3"]],"paths":[],"covariances":[],"equal":[[{"variance":{"variable":{"column":"C2"}}},{"variance":{"variable":{"column":"C1"}}}]],"missingCodes":[]}"#);
    assert_eq!((e.code, int(&e, ParamKey::Index), int(&e, ParamKey::Value)), (C::EqualOutOfOrder, Some(1), Some(2)));
    // The limits: 17 factors, 65 paths, 257 covariances, 129 members.
    let wide = table(&structured(20, MAX_COLUMNS as u64, 17));
    let mut many = SemParams { indicators: (1..=17u32).map(|f| vec![alias(f), alias(f + 17)]).collect(), paths: vec![], covariances: vec![], equal: vec![], missing_codes: vec![] };
    let r = validate(&wide, &spec(OperationId(1), &wide, many.clone()));
    assert_eq!((r.errors[0].code, int(&r.errors[0], ParamKey::Count), int(&r.errors[0], ParamKey::Limit)), (C::TooManyFactors, Some(17), Some(MAX_FACTORS as i64)));
    many.indicators.truncate(1);
    many.paths = (0..=MAX_PATHS as u32).map(|i| path(col(40 + i), col(3))).collect();
    let r = validate(&wide, &spec(OperationId(1), &wide, many.clone()));
    assert_eq!((r.errors[0].code, int(&r.errors[0], ParamKey::Count), int(&r.errors[0], ParamKey::Limit)), (C::TooManyPaths, Some(65), Some(MAX_PATHS as i64)));
    many.paths.clear();
    many.covariances = (0..=MAX_COVARIANCES as u32).map(|i| covariance(col(3 + i / 120), col(10 + i % 118))).collect();
    let r = validate(&wide, &spec(OperationId(1), &wide, many.clone()));
    assert_eq!((r.errors[0].code, int(&r.errors[0], ParamKey::Count)), (C::TooManyCovariances, Some(257)));
    many.covariances.clear();
    many.equal = vec![(0..=MAX_EQUAL_MEMBERS as u32).map(|i| variance(col(1 + i))).collect()];
    let r = validate(&wide, &spec(OperationId(1), &wide, many));
    assert_eq!((r.errors[0].code, int(&r.errors[0], ParamKey::Count)), (C::TooManyEqualMembers, Some(129)));
    // Columns: one column in all; a table holds at most 128, so 129 names one past it.
    let r = validate(&d, &spec(OperationId(1), &d, sem(r#"{"indicators":[],"paths":[],"covariances":[{"a":{"factor":1},"b":{"column":"C1"}}],"equal":[],"missingCodes":[]}"#)));
    assert_eq!(codes(&r.errors), [C::UnknownFactor.wire(), C::TooFewColumns.wire()]);
}

fn alias(n: u32) -> gaply_core::stats_desc::Alias {
    gaply_core::stats_desc::Alias(gaply_core::stats_l1::dataset::ColumnAlias(n))
}
fn col(n: u32) -> gaply_core::stats_sem::SemVariable {
    gaply_core::stats_sem::SemVariable::Column(alias(n))
}
fn path(to: gaply_core::stats_sem::SemVariable, from: gaply_core::stats_sem::SemVariable) -> gaply_core::stats_sem::SemPath {
    gaply_core::stats_sem::SemPath { to, from }
}
fn covariance(a: gaply_core::stats_sem::SemVariable, b: gaply_core::stats_sem::SemVariable) -> gaply_core::stats_sem::SemCovariance {
    gaply_core::stats_sem::SemCovariance { a, b }
}
fn variance(v: gaply_core::stats_sem::SemVariable) -> gaply_core::stats_sem::SemParameter {
    gaply_core::stats_sem::SemParameter::Variance(gaply_core::stats_sem::SemVarianceOf { variable: v })
}

#[test]
fn the_rows_checks_come_only_with_no_other_error_each_only_when_none_before_it_applies() {
    let two = sem(r#"{"indicators":[],"paths":[{"to":{"column":"C2"},"from":{"column":"C1"}}],"covariances":[],"equal":[],"missingCodes":[]}"#);
    let holes = dataset(&["x1".into(), "x2".into()], &[vec!["1".into(), "".into()], vec!["".into(), "2".into()]]);
    assert_eq!(compute(&holes, &two).unwrap_err().code, C::NoCompleteRows);
    let few = table(&[vec![1.0, 2.0], vec![2.0, 1.0]]);
    let e = compute(&few, &two).unwrap_err();
    assert_eq!((e.code, int(&e, ParamKey::Count), int(&e, ParamKey::Limit)), (C::TooFewRows, Some(2), Some(3)));
    let constant = table(&[vec![1.0, 2.0, 4.0, 5.0], vec![2.0, 2.0, 2.0, 2.0]]);
    let e = compute(&constant, &two).unwrap_err();
    assert_eq!((e.code, int(&e, ParamKey::Column)), (C::ColumnConstant, Some(2)));
    // Identification comes before the rows: a two-column factor alone on those rows.
    let alone = sem(r#"{"indicators":[["C1","C2"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#);
    assert_eq!(compute(&holes, &alone).unwrap_err().code, C::ModelNotIdentified);
}

/* ---- identification (decision 798) ----------------------------------------- */

/// The counts on known specs, each against R's reference rank by the singular
/// values of the Jacobian of vech(Σ) at random points
/// (logs/ff_identification.log): a two-column factor alone, q 4 over 3 moments,
/// rank 3; two uncorrelated two-column factors, q 8, rank 6; two uncorrelated
/// factors with three correlated errors, q 11 over 10 moments, rank 9; a loop
/// between two columns, q 4 over 3 moments, rank 3; Wheaton's Models A to D,
/// identified.
#[test]
fn the_identification_check_counts_the_constraints_a_model_lacks() {
    let d = table(&structured(60, 6, 2));
    let count = |json: &str| match compute(&d, &sem(json)) {
        Err(e) if e.code == C::ModelNotIdentified => int(&e, ParamKey::Count),
        Err(e) => panic!("{json}: {}", e.code.wire()),
        Ok(_) => None,
    };
    assert_eq!(count(r#"{"indicators":[["C1","C2"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#), Some(1));
    assert_eq!(count(r#"{"indicators":[["C1","C2"],["C3","C4"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#), Some(2));
    assert_eq!(
        count(r#"{"indicators":[["C1","C2"],["C3","C4"]],"paths":[],"covariances":[{"a":{"column":"C1"},"b":{"column":"C3"}},{"a":{"column":"C1"},"b":{"column":"C4"}},{"a":{"column":"C2"},"b":{"column":"C4"}}],"equal":[],"missingCodes":[]}"#),
        Some(2)
    );
    assert_eq!(count(r#"{"indicators":[],"paths":[{"to":{"column":"C1"},"from":{"column":"C2"}},{"to":{"column":"C2"},"from":{"column":"C1"}}],"covariances":[],"equal":[],"missingCodes":[]}"#), Some(1));
    // validate reports the same.
    let r = validate(&d, &spec(OperationId(1), &d, sem(r#"{"indicators":[["C1","C2"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#)));
    assert_eq!((codes(&r.errors), int(&r.errors[0], ParamKey::Count)), (vec![C::ModelNotIdentified.wire()], Some(1)));
    let wheaton = file(&format!("{CFA_DATA}/wheaton.csv"));
    for model in ['A', 'B', 'C', 'D'] {
        let s = wheaton_model(model);
        assert!(validate(&wheaton, &spec(OperationId(1), &wheaton, s)).errors.is_empty(), "Model {model} is identified");
    }
}

fn wheaton_model(model: char) -> SemParams {
    let covariance = if model == 'B' || model == 'C' { r#"{"a":{"column":"C1"},"b":{"column":"C3"}}"# } else { "" };
    let equal = if model == 'C' || model == 'D' {
        r#"[{"loading":{"factor":1,"column":"C2"}},{"loading":{"factor":2,"column":"C4"}}],[{"variance":{"variable":{"column":"C1"}}},{"variance":{"variable":{"column":"C3"}}}],[{"variance":{"variable":{"column":"C2"}}},{"variance":{"variable":{"column":"C4"}}}]"#
    } else {
        ""
    };
    sem(&WHEATON_A.replace(r#""covariances":[]"#, &format!(r#""covariances":[{covariance}]"#)).replace(r#""equal":[]"#, &format!(r#""equal":[{equal}]"#)))
}

/* ---- the not-computable paths and the notes ------------------------------- */

/// Walsh ±1 patterns over 16 rows: w_m(r) = −1 when the bits of r & m number odd.
fn walsh(m: u32) -> Vec<f64> {
    (0..16u32).map(|r| if (r & m).count_ones() % 2 == 1 { -1.0 } else { 1.0 }).collect()
}

#[test]
fn each_not_computable_path_writes_its_code() {
    // fixed_parameter: a first loading's S.E., C.R. and P.
    let r = compute(&example_8(), &sem(EXAMPLE_8)).unwrap();
    let fixed = Stat::NotComputable(C::FixedParameter);
    assert_eq!((r.weights[0].se, r.weights[0].cr, r.weights[0].p), (fixed, fixed, fixed));
    assert_eq!(val(r.weights[0].estimate), 1.0);
    // df_zero and cmin_zero: a path between two columns, d = 0.
    let two = table(&[walsh(1).iter().zip(walsh(2)).map(|(a, b)| a + 0.5 * b).collect(), walsh(1)]);
    let r = compute(&two, &sem(r#"{"indicators":[],"paths":[{"to":{"column":"C2"},"from":{"column":"C1"}}],"covariances":[],"equal":[],"missingCodes":[]}"#)).unwrap();
    assert_eq!(r.fit.model.df, 0);
    assert_eq!(r.fit.model.rmsea, Stat::NotComputable(C::DfZero));
    assert_eq!(r.fit.model.hoelter05, Stat::NotComputable(C::DfZero));
    assert!(r.notes.contains(&C::DfZero));
    // independence_cmin_zero: two uncorrelated Walsh columns, S diagonal, so C_b = 0; the
    // path is 0 and the model is saturated, so NFI is 1 − 0/0.
    let flat = table(&[walsh(1), walsh(2).iter().map(|x| 2.0 * x).collect()]);
    let r = compute(&flat, &sem(r#"{"indicators":[],"paths":[{"to":{"column":"C2"},"from":{"column":"C1"}}],"covariances":[],"equal":[],"missingCodes":[]}"#)).unwrap();
    assert_eq!(val(r.fit.independence.cmin), 0.0);
    assert_eq!(r.fit.model.nfi, Stat::NotComputable(C::IndependenceCminZero));
    // cmin_zero at d above 0: Walsh columns whose S is a one-factor model's exactly.
    let f = walsh(1);
    let exact: Vec<Vec<f64>> = [(4u32, 1.0, 1.0), (5, 2.0, 0.5), (6, 0.5, 2.0), (7, 1.5, 0.25)].iter().map(|(m, l, c)| f.iter().zip(walsh(*m)).map(|(x, w)| l * x + c * w).collect()).collect();
    let r = compute(&table(&exact), &sem(r#"{"indicators":[["C1","C2","C3","C4"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#)).unwrap();
    assert_eq!((r.fit.model.df, r.fit.model.cmin), (2, Stat::Value(0.0)));
    assert_eq!(r.fit.model.hoelter01, Stat::NotComputable(C::CminZero));
    // too_few_rows_for_bcc: N = p + 2, a path between two columns over four rows.
    let four = table(&[vec![1.0, 2.0, 3.0, 5.0], vec![2.0, 1.0, 4.0, 3.0]]);
    let r = compute(&four, &sem(r#"{"indicators":[],"paths":[{"to":{"column":"C2"},"from":{"column":"C1"}}],"covariances":[],"equal":[],"missingCodes":[]}"#)).unwrap();
    assert_eq!(r.rows_used, 4);
    assert_eq!(r.fit.model.bcc, Stat::NotComputable(C::TooFewRowsForBcc));
    assert_eq!(r.fit.model.mecvi, Stat::NotComputable(C::TooFewRowsForBcc));
}

/// The R golden's error_block_not_pd spec (Research OS, Phase 9C-5, decision
/// 812): the review of 9C-4's id206 shape.
const ERROR_BLOCK_NOT_PD: &str = r#"{"indicators":[["C1","C2","C3","C4"]],"paths":[{"to":{"factor":1},"from":{"column":"C5"}}],"covariances":[{"a":{"column":"C1"},"b":{"column":"C2"}},{"a":{"column":"C1"},"b":{"column":"C3"}},{"a":{"column":"C2"},"b":{"column":"C4"}},{"a":{"column":"C3"},"b":{"column":"C5"}},{"a":{"column":"C4"},"b":{"column":"C5"}}],"equal":[],"missingCodes":[]}"#;

#[test]
fn each_note_comes_in_its_order() {
    let heywood = file(&format!("{CFA_DATA}/heywood.csv"));
    let r = compute(&heywood, &sem(r#"{"indicators":[["C1","C2","C3","C4"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#)).unwrap();
    assert_eq!(r.notes, [C::NegativeVariance]);
    assert!(r.variances.iter().any(|v| val(v.estimate) < 0.0));
    let phi = file(&format!("{CFA_DATA}/phi_not_pd.csv"));
    let r = compute(&phi, &sem(r#"{"indicators":[["C1","C2"],["C3","C4"]],"paths":[],"covariances":[{"a":{"factor":1},"b":{"factor":2}}],"equal":[],"missingCodes":[]}"#)).unwrap();
    assert_eq!(r.notes, [C::CovarianceNotPositiveDefinite]);
    // Decision 812 (the review of 9C-4, M2): the block through error covariances
    // alone. One factor, so the factors' own block is 1 × 1 and positive; the
    // block of the factor and C1 to C5 is not positive definite, and no variance
    // is below 0 (the golden's error_block_not_pd, data/id206.csv, seed 4).
    let id206 = file(&format!("{SEM_DATA}/id206.csv"));
    let r = compute(&id206, &sem(ERROR_BLOCK_NOT_PD)).unwrap();
    assert_eq!(r.notes, [C::CovarianceNotPositiveDefinite, C::DfZero]);
    assert!(r.variances.iter().all(|v| val(v.estimate) > 0.0));
    assert!(val(r.variances[0].estimate) > 0.0, "the factors' own block, 1 × 1, is positive");
    let felson = file(&format!("{SEM_DATA}/felson.csv"));
    let r = compute(&felson, &sem(EXAMPLE_7)).unwrap();
    assert_eq!(r.notes, [C::Nonrecursive]);
    let hs = file(&format!("{CFA_DATA}/hs301.csv"));
    let r = compute(&hs, &sem(r#"{"indicators":[["C1","C2","C3"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#)).unwrap();
    assert_eq!(r.notes, [C::DfZero]);
    assert_eq!(r.limitations.len(), 15);
}

/* ---- invariances ----------------------------------------------------------- */

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-12)
}

#[test]
fn permuting_the_rows_and_multiplying_every_value_by_1000_leave_the_result_within_the_gates() {
    let path = format!("{SEM_DATA}/felson.csv");
    let cols = columns_of(&path);
    let base = compute(&table(&cols), &sem(EXAMPLE_7)).unwrap();
    let reversed: Vec<Vec<f64>> = cols.iter().map(|c| c.iter().rev().copied().collect()).collect();
    let r = compute(&table(&reversed), &sem(EXAMPLE_7)).unwrap();
    for (a, b) in r.weights.iter().zip(&base.weights) {
        assert!(rel(val(a.estimate), val(b.estimate)) <= 2.2e-7, "a weight after permuting");
        assert!(rel(val(a.se), val(b.se)) <= 3.7e-9);
    }
    assert!(rel(val(r.fit.model.cmin), val(base.fit.model.cmin)) <= 6.7e-11);
    let scaled: Vec<Vec<f64>> = cols.iter().map(|c| c.iter().map(|x| x * 1000.0).collect()).collect();
    let r = compute(&table(&scaled), &sem(EXAMPLE_7)).unwrap();
    for (a, b) in r.weights.iter().zip(&base.weights) {
        assert!(rel(val(a.standardized), val(b.standardized)) <= 2.2e-7, "a standardized weight after scaling");
    }
    for (a, b) in r.smc.iter().zip(&base.smc) {
        assert!(rel(val(a.value), val(b.value)) <= 2.2e-7);
    }
    for (a, b) in r.covariances.iter().zip(&base.covariances) {
        assert!(rel(val(a.correlation), val(b.correlation)) <= 2.2e-7);
    }
    assert!(rel(val(r.fit.model.cmin), val(base.fit.model.cmin)) <= 6.7e-11);
}

/// A column on another scale changes its own weights by that factor and
/// nothing a reader compares: on Example 8 with its second column times 10⁷ (a
/// loading near 10⁷) and on Example 7 with GPA times 10⁻⁹ (academic ← GPA
/// near 2.3 · 10⁷). Found by 9C-4's hunt on 9C-2's case h226 (4,354 rows, six
/// factors, columns scaled by 10^U(−2, 3)), which the CFA solves: Gauss–Jordan
/// on I − A pivoted on a loading above 1 leaves a pivot near 1/λ, so its rule
/// (a pivot at or below 10⁻¹² of the largest entry) refused every loading above
/// about 10⁶, F was undefined at every step from a loading of 1.47 · 10⁶, and
/// the run refused as sem_not_converged. Without a loop B is now built by
/// substitution; with one, I − A is inverted in natural units: the third case,
/// the golden's loop between two factors with C2 times 10⁷ (a loading near
/// 10⁷ from factor 1 into a later column), refuses as the h226 did when every
/// unit is 1, as 9C-4's mutation m_scale_one found the first two cases could
/// not (neither puts a large weight below a diagonal of I − A).
#[test]
fn a_column_on_another_scale_changes_only_its_own_weights() {
    let base = compute(&example_8(), &sem(EXAMPLE_8)).unwrap();
    let mut cols = columns_of(&format!("{CFA_DATA}/example8.csv"));
    cols[1] = cols[1].iter().map(|x| x * 1e7).collect();
    let r = compute(&table(&cols), &sem(EXAMPLE_8)).unwrap_or_else(|e| panic!("Example 8 with C2 times 1e7: {} {:?}", e.code.wire(), e.params));
    assert!(rel(val(r.weights[1].estimate), 1e7 * val(base.weights[1].estimate)) <= 2.2e-7, "C2's loading times 1e7");
    for (a, b) in r.weights.iter().zip(&base.weights) {
        assert!(rel(val(a.standardized), val(b.standardized)) <= 2.2e-7, "a standardized weight");
        if let (Stat::Value(x), Stat::Value(y)) = (a.cr, b.cr) {
            assert!(rel(x, y) <= 2.2e-7, "a C.R.");
        }
    }
    for (a, b) in r.smc.iter().zip(&base.smc) {
        assert!(rel(val(a.value), val(b.value)) <= 2.2e-7);
    }
    assert!(rel(val(r.fit.model.cmin), val(base.fit.model.cmin)) <= 6.7e-11);

    let path = format!("{SEM_DATA}/felson.csv");
    let cols = columns_of(&path);
    let base = compute(&table(&cols), &sem(EXAMPLE_7)).unwrap();
    let mut scaled = cols.clone();
    scaled[2] = scaled[2].iter().map(|x| x * 1e-9).collect();
    let r = compute(&table(&scaled), &sem(EXAMPLE_7)).unwrap_or_else(|e| panic!("Example 7 with GPA times 1e-9: {} {:?}", e.code.wire(), e.params));
    assert!(rel(val(r.weights[1].estimate), 1e9 * val(base.weights[1].estimate)) <= 2.2e-7, "academic ← GPA times 1e9");
    for (a, b) in r.weights.iter().zip(&base.weights) {
        assert!(rel(val(a.standardized), val(b.standardized)) <= 2.2e-7, "a standardized weight");
    }
    for (a, b) in r.covariances.iter().zip(&base.covariances) {
        assert!(rel(val(a.correlation), val(b.correlation)) <= 2.2e-7);
    }
    assert!(rel(val(r.fit.model.cmin), val(base.fit.model.cmin)) <= 6.7e-11);

    const LOOP: &str = r#"{"indicators":[["C1","C2","C3"],["C4","C5","C6"]],"paths":[{"to":{"factor":1},"from":{"factor":2}},{"to":{"factor":1},"from":{"column":"C7"}},{"to":{"factor":2},"from":{"factor":1}},{"to":{"factor":2},"from":{"column":"C8"}}],"covariances":[{"a":{"column":"C7"},"b":{"column":"C8"}}],"equal":[],"missingCodes":[]}"#;
    let cols = columns_of(&format!("{SEM_DATA}/loop.csv"));
    let base = compute(&table(&cols), &sem(LOOP)).unwrap();
    let mut scaled = cols.clone();
    scaled[1] = scaled[1].iter().map(|x| x * 1e7).collect();
    let r = compute(&table(&scaled), &sem(LOOP)).unwrap_or_else(|e| panic!("the loop with C2 times 1e7: {} {:?}", e.code.wire(), e.params));
    assert!(r.notes.contains(&C::Nonrecursive), "the loop case has its loop");
    assert!(rel(val(r.weights[1].estimate), 1e7 * val(base.weights[1].estimate)) <= 2.2e-7, "C2's loading times 1e7");
    for (a, b) in r.weights.iter().zip(&base.weights) {
        assert!(rel(val(a.standardized), val(b.standardized)) <= 2.2e-7, "a standardized weight");
    }
    assert!(rel(val(r.fit.model.cmin), val(base.fit.model.cmin)) <= 6.7e-11);
}

/* ---- the attempts (decision 798) ------------------------------------------- */

#[test]
fn each_attempt_reaches_example_8s_solution_and_all_three_capped_refuse() {
    let d = example_8();
    let s = sem(EXAMPLE_8);
    let first = compute_with_caps(&d, &s, [500, 500, 500]).unwrap();
    let newton = compute_with_caps(&d, &s, [1, 500, 1]).unwrap();
    let triad = compute_with_caps(&d, &s, [1, 1, 500]).unwrap();
    assert_eq!((first.attempt, newton.attempt, triad.attempt), (1, 2, 3));
    for other in [&newton, &triad] {
        for (a, b) in other.weights.iter().zip(&first.weights) {
            assert!(rel(val(a.estimate), val(b.estimate)) <= 2.2e-7);
            if let (Stat::Value(x), Stat::Value(y)) = (a.se, b.se) {
                assert!(rel(x, y) <= 3.7e-9);
            }
        }
        assert!(rel(val(other.fit.model.cmin), val(first.fit.model.cmin)) <= 6.7e-11);
    }
    println!("Example 8: attempt 1 in {} iterations, Newton's in {}, the triad's in {}", first.iterations, newton.iterations, triad.iterations);
    let e = compute_with_caps(&d, &s, [1, 1, 1]).unwrap_err();
    assert_eq!((e.code, int(&e, ParamKey::Count), int(&e, ParamKey::Limit)), (C::SemNotConverged, Some(1), Some(1)));
}

/// Decision 798: on the CFA cases' Walsh columns (one factor of three columns
/// over 32 rows) every start puts the loadings at exactly 0, where H fails, and
/// Newton's attempt never reaches a solution, so the run refuses as
/// sem_not_converged rather than reaching the point the CFA accepts. Attempt 1
/// ends at iteration 0, the triad attempt is not made (its starts are attempt
/// 1's), and the count is Newton's: it moves 9 times and then its step rounds
/// to zero length. R's prototype stood still from there to its cap of 500, as
/// Gaply did until decision 828 (Z1); Gaply now ends Newton's attempt at a step
/// of zero length, so the count is 9.
#[test]
fn the_walsh_columns_refuse_as_not_converged() {
    let cols = vec![
        vec![2.0, 0.0, 2.0, 0.0, 2.0, 0.0, 2.0, 0.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 0.0, -2.0, 0.0, -2.0, 0.0, -2.0, 0.0, -2.0],
        vec![-0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5],
        vec![0.5, 1.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5],
    ];
    let (d, s) = (table(&cols), sem(r#"{"indicators":[["C1","C2","C3"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#));
    let max = MAX_ITERATIONS as i64;
    for (caps, want, limit) in [([MAX_ITERATIONS, 0, 0], 0, max), ([0, MAX_ITERATIONS, 0], WALSH_COUNT, max), ([0, 0, MAX_ITERATIONS], 0, 0)] {
        let one = compute_with_caps(&d, &s, caps).unwrap_err();
        println!("the Walsh columns, caps {caps:?}: {} with {:?}", one.code.wire(), one.params);
        assert_eq!(one.code, C::SemNotConverged, "{caps:?}");
        assert_eq!(int(&one, ParamKey::Count), Some(want), "{caps:?}");
        // A limit of 0 with caps [0, 0, 500]: the triad attempt was not made.
        assert_eq!(int(&one, ParamKey::Limit), Some(limit), "{caps:?}");
    }
    let e = compute(&d, &s).unwrap_err();
    println!("the Walsh columns: {} with {:?}", e.code.wire(), e.params);
    assert_eq!((e.code, int(&e, ParamKey::Count), int(&e, ParamKey::Limit)), (C::SemNotConverged, Some(WALSH_COUNT), Some(MAX_ITERATIONS as i64)));
}

/// The count the run gives on the Walsh columns, 9, from Newton's attempt
/// (decision 828, Z1; measured 16 Nov 2026, 9c5c/p0_walsh_attempts_1794807172.log).
/// Before Z1 it was 500, the cap, as R's prototype gave
/// (logs/p2_sem_targets_1794724810.log), standing still to it.
const WALSH_COUNT: i64 = 9;

/// Decision 833 (R1; decision 832's finding 1): a model every attempt refuses
/// now ends as sem_not_converged with its count, where before Z1 Newton's
/// attempt stood still until the 30 s stop. 256 rows r = 0 to 255 and 128
/// columns c = 1 to 128, the cell (−1)^popcount(r & c), so S = I exactly; 16
/// factors of 8 columns in order; the paths factor k → factor k + 1 for k = 1
/// to 15 (q = 271). Newton's attempt alone, capped at 10: it ends at 7, of
/// limit 10 (in release, 0.72 s; the whole run refuses at 7 of 500 in 0.74 s,
/// 9c5d/r1_runs_1794824308.log). With Z1's check removed it runs to its cap,
/// 10. One call only: it costs seconds in a debug build.
#[test]
fn a_model_every_attempt_refuses_ends_at_newtons_count_not_its_cap() {
    let headers: Vec<String> = (1..=128).map(|j| format!("w{j}")).collect();
    let rows: Vec<Vec<String>> = (0..256u32).map(|r| (1..=128u32).map(|c| if (r & c).count_ones() % 2 == 1 { "-1" } else { "1" }.to_string()).collect()).collect();
    let d = dataset(&headers, &rows);
    // The model as the test builds it: each factor's columns, the paths as
    // (to, from) factors, and the covariances (none). The spec is written from these.
    let factors: Vec<Vec<u32>> = (0..16u32).map(|f| (1..=8u32).map(|i| 8 * f + i).collect()).collect();
    let factor_paths: Vec<(u32, u32)> = (1..=15u32).map(|k| (k + 1, k)).collect();
    let covariances: Vec<(u32, u32)> = Vec::new();
    let indicators: Vec<String> = factors.iter().map(|f| format!("[{}]", f.iter().map(|c| format!("\"C{c}\"")).collect::<Vec<_>>().join(","))).collect();
    let paths: Vec<String> = factor_paths.iter().map(|(to, from)| format!(r#"{{"to":{{"factor":{to}}},"from":{{"factor":{from}}}}}"#)).collect();
    assert!(covariances.is_empty(), "the spec below writes no covariances");
    let s = sem(&format!(r#"{{"indicators":[{}],"paths":[{}],"covariances":[],"equal":[],"missingCodes":[]}}"#, indicators.join(","), paths.join(",")));
    // The premises: S = I exactly, since every mask m from 1 to 255 sums to 0
    // over the rows: so each column's mean is 0, and columns a ≠ b, whose product
    // is mask a ^ b, are orthogonal; and q = 271, counted from the model built
    // above as Model::of counts it (Model::q is private): each factor's free
    // loadings (its columns less the first, fixed at 1), the paths, the
    // covariances, and one variance per variable, the factors and their columns
    // (decision 839, T1).
    assert!((1..=255u32).all(|m| (0..256u32).map(|r| if (r & m).count_ones() % 2 == 1 { -1i32 } else { 1 }).sum::<i32>() == 0));
    let columns: std::collections::BTreeSet<u32> = factors.iter().flatten().copied().collect();
    let free_loadings: usize = factors.iter().map(|f| f.len() - 1).sum();
    let q = free_loadings + factor_paths.len() + covariances.len() + (factors.len() + columns.len());
    assert_eq!(q, 271, "the premise: q counted from the model the test builds");
    let e = compute_with_caps(&d, &s, [0, 10, 0]).unwrap_err();
    println!("the review's model, caps [0, 10, 0]: {} with {:?}", e.code.wire(), e.params);
    assert_eq!((e.code, int(&e, ParamKey::Count), int(&e, ParamKey::Limit)), (C::SemNotConverged, Some(7), Some(10)));
}

/// The R golden's newton_relaxed spec (Research OS, Phase 9C-5, decision 812):
/// the review of 9C-4's id384 shape on data/id384.csv, seed 17.
const NEWTON_RELAXED: &str = r#"{"indicators":[["C1","C2","C3","C4"],["C5","C6"],["C7","C8","C9","C10"]],"paths":[{"to":{"column":"C11"},"from":{"factor":3}},{"to":{"column":"C13"},"from":{"column":"C11"}}],"covariances":[{"a":{"factor":2},"b":{"column":"C4"}},{"a":{"column":"C1"},"b":{"column":"C13"}}],"equal":[],"missingCodes":[]}"#;

/// Decision 812 (the review of 9C-4, M1): on id384's shape attempt 1 fails at
/// iteration 0 (H is not positive definite at Gaply's start), and Newton's
/// attempt, with Fisher scoring's relaxation of the F test, reaches R's proper
/// minimum. At a8e53eba, without the relaxation, the run refused this dataset
/// as sem_not_converged at 500 of 500 (9c5/search/m1/gaply_pre_fix.tsv); its
/// numbers are the golden's newton_relaxed case.
#[test]
fn newtons_attempt_reaches_the_proper_minimum_of_id384_at_attempt_2() {
    let d = file(&format!("{SEM_DATA}/id384.csv"));
    let s = sem(NEWTON_RELAXED);
    let first = compute_with_caps(&d, &s, [MAX_ITERATIONS, 0, 0]).unwrap_err();
    assert_eq!((first.code, int(&first, ParamKey::Count)), (C::SemNotConverged, Some(0)), "the premise: attempt 1 ends at iteration 0");
    let r = compute(&d, &s).unwrap_or_else(|e| panic!("refused: {} {:?}", e.code.wire(), e.params));
    println!("id384: attempt {}, {} iterations", r.attempt, r.iterations);
    assert_eq!(r.attempt, 2);
    assert!(r.notes.is_empty(), "{:?}", r.notes);
    assert!(r.variances.iter().all(|v| val(v.estimate) > 0.0));
}

/* ---- a CFA spec fitted by both methods ------------------------------------- */

/// On Example 8 and HolzingerSwineford1939's three correlated factors, the
/// structural model of the same measurement model gives the CFA's estimates,
/// standard errors and fit within the CFA golden's gates, at its attempt 1.
#[test]
fn a_cfa_spec_fitted_as_a_structural_model_gives_the_cfas_result() {
    use gaply_core::stats_cfa::{compute as cfa, CfaParams, Stat as CfaStat};
    let cv = |s: CfaStat| match s {
        CfaStat::Value(x) => x,
        CfaStat::NotComputable(_) => panic!("not computable"),
    };
    let cases: [(Dataset, Vec<Vec<u32>>, &str); 2] = [
        (example_8(), vec![vec![1, 2, 3], vec![4, 5, 6]], EXAMPLE_8),
        (
            file(&format!("{CFA_DATA}/hs301.csv")),
            vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9]],
            r#"{"indicators":[["C1","C2","C3"],["C4","C5","C6"],["C7","C8","C9"]],"paths":[],"covariances":[{"a":{"factor":1},"b":{"factor":2}},{"a":{"factor":1},"b":{"factor":3}},{"a":{"factor":2},"b":{"factor":3}}],"equal":[],"missingCodes":[]}"#,
        ),
    ];
    for (d, factors, json) in cases {
        let c = cfa(&d, &CfaParams { indicators: factors.iter().map(|f| f.iter().map(|n| alias(*n)).collect()).collect(), correlated_factors: true, missing_codes: vec![] }).unwrap();
        let s = compute(&d, &sem(json)).unwrap();
        assert_eq!(s.attempt, 1);
        for (a, b) in s.weights.iter().zip(&c.loadings) {
            assert!(rel(val(a.estimate), cv(b.estimate)) <= 2.1e-8, "a loading");
            // A first loading's S.E. is fixed_parameter in both; any other is a value in both
            // (the review of 9C-4: a mismatch was skipped).
            match (a.se, b.se) {
                (Stat::Value(x), CfaStat::Value(y)) => assert!(rel(x, y) <= 1.3e-8, "a loading's S.E."),
                (Stat::NotComputable(_), CfaStat::NotComputable(_)) => {}
                (x, y) => panic!("a loading's S.E.: SEM {x:?}, CFA {y:?}"),
            }
            assert!(rel(val(a.standardized), cv(b.standardized)) <= 3.3e-8);
        }
        let k = factors.len();
        for (f, v) in c.factor_variances.iter().enumerate() {
            assert!(rel(val(s.variances[f].estimate), cv(v.estimate)) <= 2.1e-8, "a factor variance");
            assert!(rel(val(s.variances[f].se), cv(v.se)) <= 1.3e-8);
        }
        for (i, e) in c.error_variances.iter().enumerate() {
            assert!(rel(val(s.variances[k + i].estimate), cv(e.estimate)) <= 2.1e-8, "an error variance");
            assert!(rel(val(s.variances[k + i].se), cv(e.se)) <= 1.3e-8, "an error variance's S.E.");
        }
        for (a, b) in s.covariances.iter().zip(&c.factor_covariances) {
            assert!(rel(val(a.estimate), cv(b.estimate)) <= 2.1e-8, "a covariance");
            assert!(rel(val(a.correlation), cv(b.correlation)) <= 3.3e-8);
        }
        assert!(rel(val(s.fit.model.cmin), cv(c.fit.model.cmin)) <= 2.2e-11, "CMIN");
        assert!(rel(val(s.fit.model.rmsea), cv(c.fit.model.rmsea)) <= 1.6e-11, "RMSEA");
        assert_eq!((s.fit.model.npar, s.fit.model.df), (c.fit.model.npar, c.fit.model.df));
    }
}

/* ---- repeated runs, keys, check points, size ------------------------------ */

#[test]
fn repeated_runs_give_the_same_bits_and_the_result_has_its_exact_keys() {
    let felson = file(&format!("{SEM_DATA}/felson.csv"));
    let a = compute(&felson, &sem(EXAMPLE_7)).unwrap();
    let b = compute(&felson, &sem(EXAMPLE_7)).unwrap();
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    let v: serde_json::Value = serde_json::to_value(&a).unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    let mut want = vec!["columns", "rowsUsed", "indicators", "weights", "covariances", "variances", "smc", "effects", "fit", "iterations", "attempt", "exclusions", "notes", "limitations"];
    let mut got = keys.clone();
    got.sort_unstable();
    want.sort_unstable();
    assert_eq!(got, want);
    let weight: Vec<&str> = v["weights"][0].as_object().unwrap().keys().map(String::as_str).collect();
    let mut weight_sorted = weight.clone();
    weight_sorted.sort_unstable();
    assert_eq!(weight_sorted, ["cr", "estimate", "from", "kind", "p", "se", "standardized", "to"]);
    assert_eq!(v["weights"][0]["to"], serde_json::json!({"column": "C1"}));
    assert_eq!(v["weights"][0]["kind"], "path");
    let effect: Vec<&str> = v["effects"][0].as_object().unwrap().keys().map(String::as_str).collect();
    let mut effect_sorted = effect.clone();
    effect_sorted.sort_unstable();
    assert_eq!(effect_sorted, ["direct", "directStandardized", "from", "indirect", "indirectStandardized", "to", "total", "totalStandardized"]);
    assert_eq!(v["variances"][0].as_object().unwrap().len(), 6);
    assert_eq!(v["fit"]["model"].as_object().unwrap().len(), 38);
    // No header reaches the result: the same data under sentinel headers, with its
    // positive control.
    let headers: Vec<String> = (1..=6).map(|j| format!("HDR_SENTINEL_{j}")).collect();
    let cols = columns_of(&format!("{SEM_DATA}/felson.csv"));
    let rows: Vec<Vec<String>> = (0..cols[0].len()).map(|r| cols.iter().map(|c| format!("{:?}", c[r])).collect()).collect();
    let sentinel = dataset(&headers, &rows);
    assert!(sentinel.schema.columns.iter().any(|c| c.header == "HDR_SENTINEL_1"), "the positive control");
    let text = serde_json::to_string(&compute(&sentinel, &sem(EXAMPLE_7)).unwrap()).unwrap();
    assert!(!text.contains("HDR_SENTINEL"), "a header in the result");
}

#[test]
fn the_check_points_come_in_their_order_and_a_stop_ends_the_run_anywhere() {
    let d = example_8();
    let s = sem(EXAMPLE_8);
    assert_eq!(check_points(&s), 1 + 6 + 4);
    let mut seen = Vec::new();
    let r = run(&d, &s, |i| {
        seen.push(i);
        ControlFlow::<()>::Continue(())
    })
    .unwrap();
    let distinct: Vec<usize> = seen.iter().copied().fold(Vec::new(), |mut acc, i| {
        if acc.last() != Some(&i) {
            acc.push(i);
        }
        acc
    });
    assert_eq!(distinct, (0..check_points(&s)).collect::<Vec<usize>>());
    let estimation = seen.iter().filter(|i| **i == 7).count() as u64;
    assert_eq!(estimation, 1 + r.iterations + 1, "one at the start, one per step, and one before the check at the solution");
    for (at, nth) in [(3usize, 1usize), (7, 2), (9, 1), (9, 2), (10, 1), (10, 2)] {
        let mut calls = 0;
        let stopped = run(&d, &s, |i| {
            if i == at {
                calls += 1;
                if calls == nth {
                    return ControlFlow::Break("stop");
                }
            }
            ControlFlow::Continue(())
        });
        assert_eq!(stopped.unwrap_err(), Halt::Stopped { reason: "stop" }, "at {at}, call {nth}");
    }
}

/// The widest result within the limits (decision 800): 128 columns, 16
/// factors of five columns and 48 columns no factor holds; 64 paths in one loop
/// through the 64 structural variables (the 16 factors and the 48 columns), so
/// 64 × 63 = 4,032 effects; 256 covariances, among the factors' columns'
/// errors, each a column with the same column of a later factor (5, 10, 15 and
/// 20 columns on). The data are drawn from that loop with every weight 0.3 by
/// splitmix64, each normal the sum of twelve uniforms less 6. Serialised, the
/// result is under 2 MiB.
#[test]
fn the_widest_result_is_under_2_mib() {
    let n = 400usize;
    let mut g = 12345u64;
    let mut normal = || {
        let mut s = 0.0;
        for _ in 0..12 {
            g = g.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = g;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            s += ((z ^ (z >> 31)) >> 11) as f64 / 9_007_199_254_740_992.0;
        }
        s - 6.0
    };
    // The 64 structural variables over n rows: η = 0.3 shift(η) + ζ, iterated to its
    // fixed point (0.3^60 is far below rounding).
    let zeta: Vec<Vec<f64>> = (0..64).map(|_| (0..n).map(|_| normal()).collect()).collect();
    let mut eta = zeta.clone();
    for _ in 0..60 {
        eta = (0..64).map(|i| (0..n).map(|r| 0.3 * eta[(i + 63) % 64][r] + zeta[i][r]).collect()).collect();
    }
    let mut cols: Vec<Vec<f64>> = Vec::new();
    for f in &eta[..16] {
        for _ in 0..5 {
            cols.push(f.iter().map(|x| x + 0.7 * normal()).collect());
        }
    }
    cols.extend(eta[16..].iter().cloned());
    // Columns 1 to 80 are the factors' (factor f on columns 5f − 4 to 5f), 81 to 128 free.
    let indicators: Vec<String> = (0..16).map(|f| format!("[{}]", (1..=5).map(|j| format!("\"C{}\"", 5 * f + j)).collect::<Vec<_>>().join(","))).collect();
    let var = |i: usize| if i < 16 { format!(r#"{{"factor":{}}}"#, i + 1) } else { format!(r#"{{"column":"C{}"}}"#, 80 + i - 15) };
    // Each variable has one path into it, from the one before it in the loop; the
    // spec's order is by `to`.
    let paths: Vec<String> = (0..64).map(|to| format!(r#"{{"to":{},"from":{}}}"#, var(to), var((to + 63) % 64))).collect();
    let mut pairs = Vec::new();
    for gap in [5u32, 10, 15, 20] {
        for a in 1..=80 - gap {
            pairs.push((a, a + gap));
        }
    }
    pairs.sort_unstable();
    pairs.truncate(256);
    let covs: Vec<String> = pairs.iter().map(|(a, b)| format!(r#"{{"a":{{"column":"C{a}"}},"b":{{"column":"C{b}"}}}}"#)).collect();
    let json = format!(r#"{{"indicators":[{}],"paths":[{}],"covariances":[{}],"equal":[],"missingCodes":[]}}"#, indicators.join(","), paths.join(","), covs.join(","));
    let s = sem(&json);
    assert_eq!((s.paths.len(), s.covariances.len(), s.columns().len()), (64, 256, MAX_COLUMNS));
    let r: SemOutcome = match compute(&table(&cols), &s) {
        Ok(r) => r,
        Err(e) => panic!("the widest model: {} {:?}", e.code.wire(), e.params),
    };
    let bytes = serde_json::to_vec(&r).unwrap().len();
    println!("the widest result: {bytes} bytes, {} effects, attempt {}, {} iterations", r.effects.len(), r.attempt, r.iterations);
    assert_eq!(r.effects.len(), 64 * 63);
    assert!(bytes < 2 * 1024 * 1024, "{bytes} bytes");
}
