//! **`gaply_core::stats_linear_regression`'s cases (Research OS, Phase 9A-1,
//! decisions 669 and 671)**, beside the R golden: the refusals, exclusions and
//! note by code and in their order; the not-computable paths on data; the
//! outcome's and predictors' order; what scaling a predictor and shifting the
//! outcome must do; one predictor's tolerance and VIF exactly 1; the check point;
//! and the result's exact keys.

use std::ops::ControlFlow;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ParamKey, ParamValue};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_linear_regression::{
    check_points, compute, run, spec, validate, Halt, LinearRegressionCode as C,
    LinearRegressionOutcome, LinearRegressionParams, Stat, MAX_PREDICTORS,
};

fn dataset(headers: &[&str], rows: &[Vec<String>]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers.iter().map(|h| h.to_string()).collect(), rows.to_vec()),
        DataFingerprint::of_bytes(format!("{headers:?}{rows:?}").as_bytes(), "linear regression cases", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn rows_of(columns: &[Vec<f64>]) -> Vec<Vec<String>> {
    (0..columns[0].len()).map(|r| columns.iter().map(|c| format!("{:?}", c[r])).collect()).collect()
}

fn a(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn params(outcome: u32, predictors: &[u32], codes: Vec<f64>) -> LinearRegressionParams {
    LinearRegressionParams {
        outcome: a(outcome),
        predictors: predictors.iter().map(|n| a(*n)).collect(),
        missing_codes: codes,
    }
}

fn v(s: Stat) -> f64 {
    match s {
        Stat::Value(x) => x,
        Stat::NotComputable(c) => panic!("not computable: {}", c.wire()),
    }
}

fn close(got: f64, want: f64, gate: f64) -> bool {
    (got - want).abs() <= gate * want.abs().max(1.0)
}

/// Twelve rows, three predictors, every value computed.
fn base_columns() -> Vec<Vec<f64>> {
    let x1 = vec![1.2, 2.5, 3.1, 4.8, 5.0, 6.3, 7.7, 8.1, 9.4, 10.2, 11.9, 12.5];
    let x2 = vec![0.5, -1.25, 2.0, 0.75, -0.5, 1.5, -2.0, 0.25, 1.0, -0.75, 2.5, -1.5];
    let x3 = vec![10.0, 12.0, 9.5, 14.0, 11.0, 13.5, 15.0, 10.5, 16.0, 12.5, 17.5, 14.5];
    let y: Vec<f64> = (0..12)
        .map(|i| 3.0 + 0.8 * x1[i] - 1.5 * x2[i] + 0.3 * x3[i] + [0.4, -0.3, 0.1, -0.6, 0.2, 0.5, -0.1, -0.4, 0.3, 0.6, -0.5, 0.2][i])
        .collect();
    vec![y, x1, x2, x3]
}

fn base() -> Dataset {
    dataset(&["y", "x1", "x2", "x3"], &rows_of(&base_columns()))
}

fn codes(errors: &[gaply_core::contracts::Diagnostic<C>]) -> Vec<&'static str> {
    errors.iter().map(|e| e.code.wire()).collect()
}

#[test]
fn the_checks_come_in_their_order_and_validate_lists_every_one() {
    // A code not finite, too many predictors, a column named twice, an unknown
    // column: all four, in decision 669's order.
    let d = base();
    let mut predictors: Vec<u32> = vec![2, 3, 2, 99];
    predictors.extend(std::iter::repeat(4).take(MAX_PREDICTORS));
    let p = params(1, &predictors, vec![f64::NAN]);
    let report = validate(&d, &spec(OperationId(1), &d, p.clone()));
    let got = codes(&report.errors);
    assert_eq!(got[0], C::MissingCodeNotFinite.wire());
    assert_eq!(got[1], C::TooManyPredictors.wire());
    assert_eq!(got[2], C::ColumnRepeated.wire());
    assert!(got[3..].iter().all(|c| *c == C::ColumnRepeated.wire() || *c == C::UnknownColumn.wire()), "{got:?}");
    assert!(got.contains(&C::UnknownColumn.wire()));
    // run reports the first.
    assert_eq!(compute(&d, &p).unwrap_err().code, C::MissingCodeNotFinite);
}

#[test]
fn a_column_named_twice_names_the_second_naming() {
    let d = base();
    let e = compute(&d, &params(1, &[2, 1], vec![])).unwrap_err();
    assert_eq!((e.code, e.params.get(&ParamKey::Column)), (C::ColumnRepeated, Some(&ParamValue::Int(1))));
}

#[test]
fn the_four_row_checks_report_at_most_one_and_in_their_order() {
    // Constant outcome AND constant predictor: only outcome_constant.
    let mut cols = base_columns();
    cols[0] = vec![5.0; 12];
    cols[2] = vec![1.0; 12];
    let d = dataset(&["y", "x1", "x2", "x3"], &rows_of(&cols));
    let report = validate(&d, &spec(OperationId(1), &d, params(1, &[2, 3, 4], vec![])));
    assert_eq!(codes(&report.errors), vec![C::OutcomeConstant.wire()]);
    // -0 and 0 are one value (Level 1's `constant`).
    let mut cols = base_columns();
    cols[2] = (0..12).map(|i| if i % 2 == 0 { 0.0 } else { -0.0 }).collect();
    let d = dataset(&["y", "x1", "x2", "x3"], &rows_of(&cols));
    let e = compute(&d, &params(1, &[2, 3, 4], vec![])).unwrap_err();
    assert_eq!((e.code, e.params.get(&ParamKey::Column)), (C::PredictorConstant, Some(&ParamValue::Int(3))));
    // Too few rows comes before a constant outcome: 4 rows for 3 predictors.
    let cols: Vec<Vec<f64>> = base_columns().into_iter().map(|c| c[..4].to_vec()).collect();
    let mut cols = cols;
    cols[0] = vec![1.0; 4];
    let d = dataset(&["y", "x1", "x2", "x3"], &rows_of(&cols));
    let e = compute(&d, &params(1, &[2, 3, 4], vec![])).unwrap_err();
    assert_eq!(e.code, C::TooFewRows);
    assert_eq!(e.params.get(&ParamKey::Count), Some(&ParamValue::Int(4)));
    assert_eq!(e.params.get(&ParamKey::Limit), Some(&ParamValue::Int(5)));
}

#[test]
fn no_predictor_is_refused() {
    let d = base();
    assert_eq!(compute(&d, &params(1, &[], vec![])).unwrap_err().code, C::NoPredictors);
}

#[test]
fn a_row_left_out_is_counted_once_under_its_first_column_without_a_value() {
    let mut rows = rows_of(&base_columns());
    rows[0][0] = String::new(); // outcome missing
    rows[1][1] = "-99".into(); // x1 a code
    rows[1][2] = String::new(); // and x2 missing: counted under x1 only
    rows[2][3] = String::new(); // x3 missing
    rows[3][1] = String::new(); // x1 missing
    rows[3][0] = "-99".into(); // and the outcome a code: counted under the outcome
    let d = dataset(&["y", "x1", "x2", "x3"], &rows);
    let r = compute(&d, &params(1, &[2, 3, 4], vec![-99.0])).unwrap();
    let got: Vec<(&str, u64, i64)> = r
        .exclusions
        .iter()
        .map(|e| {
            let Some(ParamValue::Int(c)) = e.params.get(&ParamKey::Column) else { panic!() };
            (e.code.wire(), e.count, *c)
        })
        .collect();
    assert_eq!(
        got,
        vec![(C::MissingValue.wire(), 1, 1), (C::MissingCode.wire(), 1, 1), (C::MissingCode.wire(), 1, 2), (C::MissingValue.wire(), 1, 4)]
    );
    assert_eq!(r.rows_used, 8);
}

/// RSS exactly 0 on an exact integer fit: y = 2 + 3x (the prompt's review found
/// this one).
#[test]
fn a_perfect_fit_gives_its_code_where_decision_669_says_and_numbers_elsewhere() {
    let d = dataset(&["x", "y"], &rows_of(&[vec![0.0, 3.0, 4.0], vec![2.0, 11.0, 14.0]]));
    let r = compute(&d, &params(2, &[1], vec![])).unwrap();
    let pf = Stat::NotComputable(C::PerfectFit);
    assert_eq!(v(r.anova.residual.sum_of_squares), 0.0, "RSS exactly 0");
    assert_eq!((r.model.r_squared, r.model.adjusted_r_squared), (Stat::Value(1.0), Stat::Value(1.0)));
    assert_eq!(r.model.standard_error_of_estimate, pf);
    assert_eq!(r.model.durbin_watson, pf);
    assert_eq!((r.anova.residual.mean_square, r.anova.f, r.anova.p), (pf, pf, pf));
    let i = &r.intercept;
    assert_eq!([i.standard_error, i.t, i.p, i.lower, i.upper], [pf; 5]);
    // The coefficients are the QR's, within rounding of 2 and 3; RSS is exactly 0
    // all the same, since the residuals come from the effects beyond the rank.
    assert!(close(v(i.b), 2.0, 1e-14), "{:?}", i.b);
    let p = &r.predictors[0];
    assert_eq!([p.standard_error, p.t, p.p, p.lower, p.upper, p.partial, p.part], [pf; 7]);
    assert!(close(v(p.b), 3.0, 1e-14), "{:?}", p.b);
    assert!(matches!(p.beta, Stat::Value(_)) && matches!(p.zero_order, Stat::Value(_)));
    assert_eq!((p.tolerance, p.vif), (Stat::Value(1.0), Stat::Value(1.0)));
    assert_eq!((r.residuals.standardized_residual.minimum, r.residuals.standardized_residual.maximum), (pf, pf));
    // R's summary.lm warns here too: its residual variance 0 is below the threshold.
    assert_eq!(r.notes, vec![C::EssentiallyPerfectFit]);
}

#[test]
fn a_well_fitted_model_has_no_note_and_every_number() {
    let r = compute(&base(), &params(1, &[2, 3, 4], vec![])).unwrap();
    assert!(r.notes.is_empty());
    let json = serde_json::to_string(&r).unwrap();
    assert!(!json.contains("not_computable"), "{json}");
}

#[test]
fn the_predictors_order_permutes_the_coefficients_and_intervals_with_it() {
    let d = base();
    let r = compute(&d, &params(1, &[2, 3, 4], vec![])).unwrap();
    let s = compute(&d, &params(1, &[4, 2, 3], vec![])).unwrap();
    for (j, k) in [(0, 1), (1, 2), (2, 0)] {
        let (p, q) = (&r.predictors[j], &s.predictors[k]);
        assert_eq!(p.column, q.column);
        for (x, y) in [(p.b, q.b), (p.standard_error, q.standard_error), (p.lower, q.lower), (p.upper, q.upper), (p.t, q.t), (p.vif, q.vif)] {
            assert!(close(v(x), v(y), 1e-12), "{x:?} {y:?}");
        }
    }
    assert!(close(v(r.intercept.b), v(s.intercept.b), 1e-12));
    assert_eq!(s.columns.iter().map(|c| c.column).collect::<Vec<_>>(), vec![a(1), a(4), a(2), a(3)]);
}

#[test]
fn the_outcome_is_the_columns_first_and_the_result_names_it() {
    let d = base();
    let r = compute(&d, &params(3, &[1, 2], vec![])).unwrap();
    assert_eq!(r.outcome, a(3));
    assert_eq!(r.columns[0].column, a(3));
}

#[test]
fn the_rows_order_changes_nothing_but_the_durbin_watson_statistic() {
    let cols = base_columns();
    let order = [7, 2, 11, 0, 5, 9, 3, 10, 1, 8, 6, 4];
    let shuffled: Vec<Vec<f64>> = cols.iter().map(|c| order.iter().map(|i| c[*i]).collect()).collect();
    let r = compute(&base(), &params(1, &[2, 3, 4], vec![])).unwrap();
    let s = compute(&dataset(&["y", "x1", "x2", "x3"], &rows_of(&shuffled)), &params(1, &[2, 3, 4], vec![])).unwrap();
    let flat = |o: &LinearRegressionOutcome| {
        let mut j: serde_json::Value = serde_json::to_value(o).unwrap();
        j["model"]["durbinWatson"] = serde_json::Value::Null;
        let mut out = Vec::new();
        numbers(&j, "", &mut out);
        out
    };
    let (a_, b_) = (flat(&r), flat(&s));
    assert_eq!(a_.len(), b_.len());
    for ((k, x), (_, y)) in a_.iter().zip(&b_) {
        assert!(close(*x, *y, 1e-11), "{k}: {x} {y}");
    }
    assert_ne!(r.model.durbin_watson, s.model.durbin_watson, "the Durbin-Watson statistic takes the rows' order");
}

fn numbers(j: &serde_json::Value, path: &str, out: &mut Vec<(String, f64)>) {
    match j {
        serde_json::Value::Number(n) => out.push((path.to_string(), n.as_f64().unwrap())),
        serde_json::Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| numbers(x, &format!("{path}.{i}"), out)),
        serde_json::Value::Object(o) => o.iter().for_each(|(k, x)| numbers(x, &format!("{path}.{k}"), out)),
        _ => {}
    }
}

#[test]
fn scaling_a_predictor_divides_its_b_and_keeps_its_t_p_and_beta() {
    let c = 250.0;
    let mut cols = base_columns();
    cols[2] = cols[2].iter().map(|x| x * c).collect();
    let r = compute(&base(), &params(1, &[2, 3, 4], vec![])).unwrap();
    let s = compute(&dataset(&["y", "x1", "x2", "x3"], &rows_of(&cols)), &params(1, &[2, 3, 4], vec![])).unwrap();
    let (p, q) = (&r.predictors[1], &s.predictors[1]);
    assert!(close(v(q.b), v(p.b) / c, 1e-12));
    for (x, y) in [(p.t, q.t), (p.p, q.p), (p.beta, q.beta), (p.vif, q.vif), (p.partial, q.partial)] {
        assert!(close(v(x), v(y), 1e-12), "{x:?} {y:?}");
    }
    assert!(close(v(r.model.r_squared), v(s.model.r_squared), 1e-12));
}

#[test]
fn shifting_the_outcome_moves_the_intercept_its_mean_and_the_predicted_values_only() {
    let c = 1000.0;
    let mut cols = base_columns();
    cols[0] = cols[0].iter().map(|x| x + c).collect();
    let r = compute(&base(), &params(1, &[2, 3, 4], vec![])).unwrap();
    let s = compute(&dataset(&["y", "x1", "x2", "x3"], &rows_of(&cols)), &params(1, &[2, 3, 4], vec![])).unwrap();
    assert!(close(v(s.intercept.b), v(r.intercept.b) + c, 1e-12));
    assert!(close(v(s.columns[0].mean), v(r.columns[0].mean) + c, 1e-12));
    for (x, y) in [(r.residuals.predicted.minimum, s.residuals.predicted.minimum), (r.residuals.predicted.maximum, s.residuals.predicted.maximum), (r.residuals.predicted.mean, s.residuals.predicted.mean)] {
        assert!(close(v(y), v(x) + c, 1e-12));
    }
    let mut a_ = serde_json::to_value(&r).unwrap();
    let mut b_ = serde_json::to_value(&s).unwrap();
    for j in [&mut a_, &mut b_] {
        j["intercept"]["b"] = serde_json::Value::Null;
        j["intercept"]["lower"] = serde_json::Value::Null;
        j["intercept"]["upper"] = serde_json::Value::Null;
        j["intercept"]["t"] = serde_json::Value::Null;
        j["intercept"]["p"] = serde_json::Value::Null;
        j["columns"][0]["mean"] = serde_json::Value::Null;
        for k in ["minimum", "maximum", "mean"] {
            j["residuals"]["predicted"][k] = serde_json::Value::Null;
        }
    }
    let (mut x, mut y) = (Vec::new(), Vec::new());
    numbers(&a_, "", &mut x);
    numbers(&b_, "", &mut y);
    assert_eq!(x.len(), y.len());
    // The rest is unmoved, within a gate scaled to c: the residuals are computed
    // from a fit whose effects carry c.
    for ((k, p), (_, q)) in x.iter().zip(&y) {
        // Decision 671: within the coefficient-scale gate of linear_regression_r_golden.rs (6e-11).
        assert!((p - q).abs() <= 6e-11 * p.abs().max(1.0), "{k}: {p} {q}");
    }
}

#[test]
fn one_predictor_has_tolerance_and_vif_exactly_1() {
    let d = base();
    for j in [2, 3, 4] {
        let r = compute(&d, &params(1, &[j], vec![])).unwrap();
        assert_eq!((r.predictors[0].tolerance, r.predictors[0].vif), (Stat::Value(1.0), Stat::Value(1.0)));
    }
    // Two predictors: from the arithmetic, above 1.
    let r = compute(&d, &params(1, &[2, 4], vec![])).unwrap();
    assert!(v(r.predictors[0].vif) > 1.0 && v(r.predictors[0].tolerance) == 1.0 / v(r.predictors[0].vif));
}

#[test]
fn the_run_has_one_check_point_before_the_arithmetic() {
    assert_eq!(check_points(), 1);
    let d = base();
    let mut seen = Vec::new();
    let got = run(&d, &params(1, &[2, 3], vec![]), |i| {
        seen.push(i);
        ControlFlow::Break("stop")
    });
    assert_eq!(got.unwrap_err(), Halt::Stopped { reason: "stop" });
    assert_eq!(seen, vec![0]);
    // A refused spec never reaches it.
    let mut reached = false;
    let got = run(&d, &params(1, &[], vec![]), |_| {
        reached = true;
        ControlFlow::<()>::Continue(())
    });
    assert!(matches!(got, Err(Halt::Refused(_))) && !reached);
}

#[test]
fn the_result_has_exactly_its_keys() {
    let r = compute(&base(), &params(1, &[2, 3], vec![])).unwrap();
    let j = serde_json::to_value(&r).unwrap();
    let keys = |v: &serde_json::Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys(&j), ["anova", "columns", "exclusions", "intercept", "limitations", "model", "notes", "outcome", "predictors", "residuals", "rowsUsed"]);
    assert_eq!(keys(&j["columns"][0]), ["column", "mean", "sd"]);
    assert_eq!(keys(&j["model"]), ["adjustedRSquared", "durbinWatson", "r", "rSquared", "standardErrorOfEstimate"]);
    assert_eq!(keys(&j["anova"]), ["f", "p", "regression", "residual", "total"]);
    assert_eq!(keys(&j["anova"]["regression"]), ["df", "meanSquare", "sumOfSquares"]);
    assert_eq!(keys(&j["anova"]["total"]), ["df", "sumOfSquares"]);
    assert_eq!(keys(&j["intercept"]), ["b", "lower", "p", "standardError", "t", "upper"]);
    assert_eq!(
        keys(&j["predictors"][0]),
        ["b", "beta", "column", "lower", "p", "part", "partial", "standardError", "t", "tolerance", "upper", "vif", "zeroOrder"]
    );
    assert_eq!(keys(&j["residuals"]), ["predicted", "residual", "standardizedPredicted", "standardizedResidual"]);
    assert_eq!(keys(&j["residuals"]["predicted"]), ["maximum", "mean", "minimum", "standardDeviation"]);
    assert_eq!(keys(&j["residuals"]["standardizedResidual"]), ["maximum", "minimum"]);
    assert_eq!(j["anova"]["regression"]["df"], 2);
    assert_eq!(j["anova"]["residual"]["df"], 9);
    assert_eq!(j["anova"]["total"]["df"], 11);
    let limitations: Vec<&str> = r.limitations.iter().map(|c| c.wire()).collect();
    assert_eq!(
        limitations,
        [
            "stats.linear_regression.enter_only",
            "stats.linear_regression.no_categorical_predictors",
            "stats.linear_regression.no_interaction_terms",
            "stats.linear_regression.intercept_always",
            "stats.linear_regression.listwise_only",
            "stats.linear_regression.no_robust_standard_errors",
            "stats.linear_regression.no_casewise_diagnostics",
            "stats.linear_regression.durbin_watson_row_order",
            "stats.linear_regression.intervals_95_only",
            "stats.linear_regression.no_weights",
        ]
    );
}

#[test]
fn a_column_of_text_or_an_infinity_is_refused_by_its_alias_and_count() {
    let mut rows = rows_of(&base_columns());
    rows[3][2] = "abc".into();
    rows[5][2] = "def".into();
    let d = dataset(&["y", "x1", "x2", "x3"], &rows);
    let e = compute(&d, &params(1, &[2, 3], vec![])).unwrap_err();
    assert_eq!((e.code, e.params.get(&ParamKey::Column), e.params.get(&ParamKey::Count)), (C::ColumnHoldsText, Some(&ParamValue::Int(3)), Some(&ParamValue::Int(2))));
}

#[test]
fn every_wording_this_module_holds_is_a_code_and_none_is_a_verdict() {
    // The closed codes are the only words the result carries.
    let r = compute(&base(), &params(1, &[2, 3, 4], vec![])).unwrap();
    let json = serde_json::to_string(&r).unwrap().to_lowercase();
    for w in ["significan", "should", "recommend", "valid", "accurate"] {
        assert!(!json.contains(w), "{w}");
    }
}

/// The hunt's h204 (decision 671): a coefficient's σ² · s_j below the smallest
/// normal double holds only a few significant bits, so its √ is a finite, wrong
/// standard error, and R's lm printed t −8.70 where the exact t is −12.31. Gaply
/// gives `overflow` there, never the subnormal's number; a control at a scale
/// where the product is normal keeps the scale-free t.
#[test]
fn a_standard_error_from_a_subnormal_product_is_not_computable_never_a_wrong_number() {
    let cols = base_columns();
    let at = |sx: f64, sy: f64| {
        let scaled = vec![cols[0].iter().map(|v| v * sy).collect::<Vec<f64>>(), cols[1].iter().map(|v| v * sx).collect()];
        compute(&dataset(&["y", "x1"], &rows_of(&scaled)), &params(1, &[2], vec![])).unwrap()
    };
    let plain = at(1.0, 1.0);
    let t_plain = v(plain.predictors[0].t);
    // Control: σ² near 1e-120 and s_j near 1e-150, a product near 1e-270, normal.
    let normal = at(1e75, 1e-60);
    assert!(close(v(normal.predictors[0].t), t_plain, 1e-12), "{:?} {t_plain}", normal.predictors[0].t);
    // σ² near 1e-144 and s_j near 1e-172: a product near 1e-316, subnormal.
    let subnormal = at(1e85, 1e-72);
    let p = &subnormal.predictors[0];
    let o = Stat::NotComputable(C::Overflow);
    assert_eq!([p.standard_error, p.t, p.p, p.lower, p.upper, p.partial, p.part], [o; 7], "t {:?} where the scale-free t is {t_plain}", p.t);
    // The coefficient itself is computed, and the intercept's SE is normal.
    assert!(matches!(p.b, Stat::Value(_)) && matches!(subnormal.intercept.standard_error, Stat::Value(_)));
}

/// The note's threshold is R's (decision 669): R 4.6.1's summary.lm does not warn
/// on this fit, whose residual variance over (mean(fitted)² + var(fitted)) is
/// 2.07e-26, above its 1e-30 (measured in this session,
/// ~/gaply-corpus/os/phase9/9a1/near_perfect_r.log); y is R's doubles at %.17g.
#[test]
fn a_near_perfect_fit_above_rs_threshold_has_no_note() {
    let y = [
        "100002.00000001",
        "100003.99999998001",
        "100006.000000015",
        "100007.99999999499",
        "100010.00000001999",
        "100011.99999999",
        "100014.00000000501",
        "100015.999999985",
        "100018.00000001",
        "100019.99999999",
    ];
    let rows: Vec<Vec<String>> = y.iter().enumerate().map(|(i, v)| vec![v.to_string(), (i + 1).to_string()]).collect();
    let r = compute(&dataset(&["y", "x"], &rows), &params(1, &[2], vec![])).unwrap();
    assert!(r.notes.is_empty(), "{:?}", r.notes);
    assert!(matches!(r.model.standard_error_of_estimate, Stat::Value(s) if s > 0.0));
}

/// The review of 9A-1 (finding 1): a σ² = RSS / df below the smallest normal
/// double, with RSS itself normal, keeps a few significant bits, so σ, the
/// residual mean square, F, every SE and the standardized residuals built on it
/// would be finite and imprecise. Each is `overflow`; so is the residuals' SD,
/// whose variance RSS / (n − 1) is subnormal too. The coefficients, R² and the
/// predicted values stay numbers.
#[test]
fn a_subnormal_residual_variance_is_not_computable_never_a_wrong_number() {
    let cols = base_columns();
    let base = compute(&base(), &params(1, &[2, 3, 4], vec![])).unwrap();
    let rss = v(base.anova.residual.sum_of_squares);
    // Scaled so RSS is about 4 times the smallest normal double: σ² = RSS / 8 is subnormal.
    let c = (4.0 * f64::MIN_POSITIVE / rss).sqrt();
    let mut scaled = cols.clone();
    scaled[0] = cols[0].iter().map(|y| y * c).collect();
    let r = compute(&dataset(&["y", "x1", "x2", "x3"], &rows_of(&scaled)), &params(1, &[2, 3, 4], vec![])).unwrap();
    let held = v(r.anova.residual.sum_of_squares);
    assert!(held >= f64::MIN_POSITIVE && held / 8.0 < f64::MIN_POSITIVE, "the premise: RSS {held:e} normal, σ² subnormal");
    let o = Stat::NotComputable(C::Overflow);
    assert_eq!(r.model.standard_error_of_estimate, o);
    assert_eq!((r.anova.residual.mean_square, r.anova.f, r.anova.p), (o, o, o));
    assert_eq!([r.intercept.standard_error, r.intercept.t, r.intercept.p], [o; 3]);
    for p in &r.predictors {
        assert_eq!([p.standard_error, p.t, p.p, p.lower, p.upper, p.partial, p.part], [o; 7], "{:?}", p.column);
        assert!(matches!(p.b, Stat::Value(_)));
    }
    assert_eq!((r.residuals.standardized_residual.minimum, r.residuals.standardized_residual.maximum), (o, o));
    assert_eq!(r.residuals.residual.standard_deviation, o);
    assert!(matches!(r.model.r_squared, Stat::Value(_)) && matches!(r.residuals.predicted.mean, Stat::Value(_)));
}

/// The review of 9A-1 (finding 5): the outcome's column error comes before a
/// predictor's, whatever the errors are.
#[test]
fn the_outcomes_column_error_comes_before_a_predictors() {
    let mut rows = rows_of(&base_columns());
    rows[2][0] = "abc".into(); // the outcome holds text
    rows[4][1] = "inf".into(); // the first predictor holds an infinity
    let d = dataset(&["y", "x1", "x2", "x3"], &rows);
    let report = validate(&d, &spec(OperationId(1), &d, params(1, &[2, 3], vec![])));
    assert_eq!(codes(&report.errors), vec![C::ColumnHoldsText.wire(), C::ColumnNonFinite.wire()]);
    assert_eq!(report.errors[0].params.get(&ParamKey::Column), Some(&ParamValue::Int(1)));
    assert_eq!(report.errors[1].params.get(&ParamKey::Column), Some(&ParamValue::Int(2)));
}
