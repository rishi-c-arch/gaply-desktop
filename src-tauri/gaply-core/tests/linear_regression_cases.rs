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

/// The split forms' gate against gmp's exact values (decision 683): ten times the
/// largest relative error measured over the split-form tests, 1.595e-15 (the
/// subnormal σ² test; 6.18e-16 for the split SE, 1.394e-15 for F's split form;
/// macOS aarch64, 11 Nov 2026), rounded up.
const SPLIT_GATE: f64 = 1.6e-14;

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

/// The hunt's h204 (decisions 671 and 683): when σ² s_j is subnormal, or overflows,
/// while the standard error itself is an ordinary double, the SE is σ √s_j, whose
/// intermediates stay normal, and t, the interval, partial and part follow from it.
/// 9A-1 withheld these as `overflow`; the review of d04199c4 showed the split form
/// gives the exact t (R's lm printed −8.70 for h204's predictor, where the exact t
/// is −12.31). The data are the base model's first predictor, scaled by powers of
/// two, so every exact value is the base value (gmp, `make_exact_cases.R`) times an
/// exact power of two; each is compared by relative error.
#[test]
fn a_standard_error_whose_product_is_subnormal_or_overflows_is_computed_in_its_split_form() {
    let cols = base_columns();
    let at = |sx: i32, sy: i32| {
        let (fx, fy) = (2f64.powi(sx), 2f64.powi(sy));
        let scaled = vec![cols[0].iter().map(|v| v * fy).collect::<Vec<f64>>(), cols[1].iter().map(|v| v * fx).collect()];
        compute(&dataset(&["y", "x1"], &rows_of(&scaled)), &params(1, &[2], vec![])).unwrap()
    };
    // gmp's exact base values (one predictor).
    let (se1, t1, partial1, se0, b1) = (0.18550453958047611, 5.2792024780743061, 0.85786863213105091, 1.4371015649294858, 0.97931602504728266);
    let mut worst = 0.0f64;
    for (sx, sy, product) in [(282, -239, "subnormal"), (-332, 332, "infinite")] {
        let r = at(sx, sy);
        let p = &r.predictors[0];
        let scale_se = 2f64.powi(sy - sx);
        for (got, exact) in [
            (v(p.standard_error), se1 * scale_se),
            (v(p.t), t1),
            (v(p.partial), partial1),
            (v(p.part), partial1),
            (v(p.b), b1 * scale_se),
            (v(r.intercept.standard_error), se0 * 2f64.powi(sy)),
        ] {
            worst = worst.max(((got - exact) / exact).abs());
        }
        assert!(matches!(p.p, Stat::Value(_)) && matches!(p.lower, Stat::Value(_)) && matches!(p.upper, Stat::Value(_)), "{product}: {:?}", p);
        assert!(v(p.lower) <= v(p.b) && v(p.b) <= v(p.upper), "{product}");
    }
    println!("split SE: the largest relative error against gmp is {worst:e}");
    assert!(worst <= SPLIT_GATE, "{worst:e} over {SPLIT_GATE:e}");
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

/// Decision 683: σ² = RSS / df below the smallest normal double, with RSS itself
/// normal. The residual mean square is σ² itself, a subnormal, so it is
/// `too_small`; σ is √RSS / √df, an ordinary double, and every SE, t, F, the
/// standardized residuals and the residuals' SD (by a scaled sum) are numbers. 9A-1
/// withheld all of them as `overflow`, though σ² = 1.11e-308 holds 51 significant
/// bits (the review of d04199c4). The outcome is the base model's scaled by 2^-511,
/// so RSS is 1.61 times the smallest normal double; each value is compared with
/// gmp's exact base value times its exact power of two, by relative error.
#[test]
fn a_subnormal_residual_variance_is_too_small_and_sigma_is_split() {
    let cols = base_columns();
    let mut scaled = cols.clone();
    let fy = 2f64.powi(-511);
    scaled[0] = cols[0].iter().map(|y| y * fy).collect();
    let r = compute(&dataset(&["y", "x1", "x2", "x3"], &rows_of(&scaled)), &params(1, &[2, 3, 4], vec![])).unwrap();
    let rss = v(r.anova.residual.sum_of_squares);
    assert!(rss >= f64::MIN_POSITIVE && rss / 8.0 < f64::MIN_POSITIVE, "the premise: RSS {rss:e} normal, σ² subnormal");
    assert_eq!(r.anova.residual.mean_square, Stat::NotComputable(C::TooSmall));
    // gmp's exact base values (three predictors).
    let sigma = 0.44926285760588763;
    let ses = [0.79156070219473562, 0.052025911757214015, 0.096308914108251681, 0.076566612208272142];
    let ts = [4.6998071882941348, 16.015874901052829, -15.749083436615184, 3.0014724298618742];
    let mut worst = 0.0f64;
    let mut see = |got: f64, exact: f64| worst = worst.max(((got - exact) / exact).abs());
    see(v(r.model.standard_error_of_estimate), sigma * fy);
    see(v(r.anova.f), 320.69901932087345);
    see(v(r.residuals.residual.standard_deviation), 0.38313265229417542 * fy);
    see(v(r.residuals.standardized_residual.maximum), 1.2088649527904383);
    see(v(r.residuals.standardized_residual.minimum), -1.4429602488778805);
    see(v(r.intercept.standard_error), ses[0] * fy);
    see(v(r.intercept.t), ts[0]);
    for (j, p) in r.predictors.iter().enumerate() {
        see(v(p.standard_error), ses[j + 1] * fy);
        see(v(p.t), ts[j + 1]);
        assert!(matches!(p.p, Stat::Value(_)) && matches!(p.partial, Stat::Value(_)) && matches!(p.part, Stat::Value(_)));
    }
    println!("subnormal σ²: the largest relative error against gmp is {worst:e}");
    assert!(worst <= SPLIT_GATE, "{worst:e} over {SPLIT_GATE:e}");
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

/// Decision 683: the regression mean square MSS / k subnormal while MSS is normal:
/// it is `too_small`, and F is (MSS / RSS)(df / k), an ordinary double; the
/// predicted values' SD, whose variance is subnormal, comes from a scaled sum. The
/// outcome is a weak one (R² 0.063) scaled by 2^-508, so MSS is 2.32 times the
/// smallest normal double; exact values from gmp (`make_exact_cases.R`).
#[test]
fn a_subnormal_regression_mean_square_is_too_small_and_f_is_split() {
    let cols = base_columns();
    let weak = [4.9, 5.2, 5.1, 4.7, 5.3, 5.0, 4.8, 5.2, 5.0, 4.6, 5.1, 5.3];
    let fy = 2f64.powi(-508);
    let scaled = vec![weak.iter().map(|y| y * fy).collect::<Vec<f64>>(), cols[1].clone(), cols[2].clone(), cols[3].clone()];
    let r = compute(&dataset(&["y", "x1", "x2", "x3"], &rows_of(&scaled)), &params(1, &[2, 3, 4], vec![])).unwrap();
    let mss = v(r.anova.regression.sum_of_squares);
    assert!(mss >= f64::MIN_POSITIVE && mss / 3.0 < f64::MIN_POSITIVE, "the premise: MSS {mss:e} normal, MSS / 3 subnormal");
    assert_eq!(r.anova.regression.mean_square, Stat::NotComputable(C::TooSmall));
    let mut worst = 0.0f64;
    let mut see = |got: f64, exact: f64| worst = worst.max(((got - exact) / exact).abs());
    see(v(r.anova.f), 0.17923123498756452);
    see(v(r.residuals.predicted.standard_deviation), 0.057459689198772659 * fy);
    see(v(r.model.standard_error_of_estimate), 0.25989153794315129 * fy);
    assert!(matches!(r.anova.p, Stat::Value(p) if (0.0..=1.0).contains(&p)));
    println!("subnormal MSS / k: the largest relative error against gmp is {worst:e}");
    assert!(worst <= SPLIT_GATE, "{worst:e} over {SPLIT_GATE:e}");
}

/// Decision 683: predicted values with no spread. Each outcome here has an exact
/// covariance of 0 with its predictor (rows in mirrored pairs, checked with gmp by
/// `make_nospread.R`, seed 20260683, ids 241 and 237). In the first MSS is exactly
/// 0, where 9A-1 gave 0/0 as `overflow`; in the second the fitted values' spread is
/// rounding, where 9A-1 gave a standardized predicted maximum of 1.278. Both now
/// give `predicted_no_spread`; b, beta, F and p stay numbers, as R gives them.
#[test]
fn predicted_values_with_no_spread_give_their_own_code() {
    let no_spread = Stat::NotComputable(C::PredictedNoSpread);
    for (y, x, mss_zero) in [
        (["1000005", "1000002", "1000001", "1000002", "1000005"], ["4", "5", "3", "1", "2"], true),
        (["5", "2", "1", "2", "5"], ["1004", "1005", "1003", "1001", "1002"], false),
    ] {
        let rows: Vec<Vec<String>> = (0..5).map(|i| vec![y[i].to_string(), x[i].to_string()]).collect();
        let r = compute(&dataset(&["y", "x"], &rows), &params(1, &[2], vec![])).unwrap();
        assert_eq!(v(r.anova.regression.sum_of_squares) == 0.0, mss_zero, "the premise");
        assert_eq!((r.residuals.standardized_predicted.minimum, r.residuals.standardized_predicted.maximum), (no_spread, no_spread), "{y:?}");
        for s in [r.predictors[0].b, r.predictors[0].beta, r.anova.f, r.anova.p, r.residuals.predicted.standard_deviation] {
            assert!(matches!(s, Stat::Value(_)), "{s:?}");
        }
    }
}

/// Decision 683(b): the measured set's closest signal case (id 17766: two
/// predictors, 20 rows, an outcome near 1e8, gmp's exact R² 1.04e-12, S = 45.9
/// against the constant 8) keeps its standardized predicted values.
#[test]
fn a_signal_at_the_measured_edge_keeps_its_standardized_predicted_values() {
    let y = "100000003.9999993,100000003.99999858,100000002.00000142,100000004.0000007,100000002.00000142,100000002.0000007,100000002.0000007,100000002,100000001.9999993,100000004.00000142,100000003.99999858,100000001.99999858,100000002,100000004.00000142,100000001.99999858,100000002,100000004.0000007,100000001.9999993,100000002,100000003.9999993";
    let x1 = "1000002,1000001,1000005,1000004,1000005,1000004,1000004,1000003,1000002,1000005,1000001,1000001,1000003,1000005,1000001,1000003,1000004,1000002,1000003,1000002";
    let x2 = "1000003,1000004,1000005,1000003,1000001,1000002,1000004,1000002,1000004,1000004,1000002,1000005,1000004,1000002,1000001,1000004,1000003,1000002,1000002,1000003";
    let cols: Vec<Vec<&str>> = [y, x1, x2].iter().map(|c| c.split(',').collect()).collect();
    let rows: Vec<Vec<String>> = (0..20).map(|i| cols.iter().map(|c| c[i].to_string()).collect()).collect();
    let r = compute(&dataset(&["y", "x1", "x2"], &rows), &params(1, &[2, 3], vec![])).unwrap();
    assert!(matches!(r.residuals.standardized_predicted.minimum, Stat::Value(_)));
    assert!(matches!(r.residuals.standardized_predicted.maximum, Stat::Value(_)));
}

/// The note's rule, pinned at both sides of R's threshold (decision 683, the review's
/// finding 4). y = 1e6 + 0.37x + 0.1 for x = 1 to 12 is an exact fit on paper with an
/// outcome mean of 1e6: R 4.6.1's summary.lm warns (ratio 2.33e-31, its own fit);
/// dropping mean² from the rule would miss it. The same with ±2e-9 alternating is just
/// above 1e-30 (R's ratio 5.31e-30, from R's own fit of y as R builds it; the review of
/// d04199c4 quoted 1.28e-33 and 4.39e-30, measured another way): R does not warn, and a factor of 1e-28 would. y is
/// built here with R's operations and checked against R's doubles first.
#[test]
fn the_note_is_rs_on_an_offset_exact_fit_and_just_above_its_threshold() {
    let offset: Vec<f64> = (1..=12).map(|x| 1e6 + 0.37 * x as f64 + 0.1).collect();
    let above: Vec<f64> = offset.iter().enumerate().map(|(i, y)| y + 2e-9 * if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
    let r_offset = ["1000000.47", "1000000.84", "1000001.21", "1000001.58", "1000001.95", "1000002.3199999999", "1000002.6899999999", "1000003.0599999999", "1000003.4299999999", "1000003.7999999999", "1000004.1699999999", "1000004.5399999999"];
    let r_above = ["1000000.470000002", "1000000.839999998", "1000001.2100000019", "1000001.579999998", "1000001.9500000019", "1000002.319999998", "1000002.6900000019", "1000003.059999998", "1000003.4300000019", "1000003.799999998", "1000004.1700000019", "1000004.5399999979"];
    for (built, r) in [(&offset, r_offset), (&above, r_above)] {
        for (b, s) in built.iter().zip(r) {
            assert_eq!(*b, s.parse::<f64>().unwrap(), "the premise: R's doubles");
        }
    }
    let fit = |y: &[f64]| {
        let rows: Vec<Vec<String>> = y.iter().enumerate().map(|(i, v)| vec![format!("{v:?}"), (i + 1).to_string()]).collect();
        compute(&dataset(&["y", "x"], &rows), &params(1, &[2], vec![])).unwrap()
    };
    assert_eq!(fit(&offset).notes, vec![C::EssentiallyPerfectFit], "R warns");
    assert!(fit(&above).notes.is_empty(), "R does not warn");
}

/// 9A-1's leftover (decision 683): a ticked code equal to a score's value is checked
/// after every column error, here after the outcome's text.
#[test]
fn a_code_equal_to_a_scores_value_comes_after_the_column_errors() {
    use gaply_core::contracts::CapabilityId;
    use gaply_core::stats_prep::{build, MissingRule, Recipe, RecipeStep, ScaleScoreStep, Statistic, CAPABILITY_ID, CAPABILITY_VERSION, ENGINE_VERSION, RECIPE_SCHEMA_VERSION};
    let mut rows = rows_of(&base_columns());
    rows[3][0] = "abc".into();
    let source = dataset(&["y", "x1", "x2", "x3"], &rows);
    let recipe = Recipe {
        schema_version: RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(CAPABILITY_ID).unwrap(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        source_columns: 4,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: a(5),
            name: "sum".into(),
            items: vec![a(2), a(3)],
            reversed: Vec::new(),
            scale: None,
            statistic: Some(Statistic::Sum),
            rule: Some(MissingRule::EveryItem),
            missing_codes: Vec::new(),
        })],
    };
    let d = build(&source, &recipe, 1_000_000).expect("prepared").dataset;
    let score_value = match &d.column_cells(ColumnAlias(5)).expect("the score")[0] {
        gaply_core::stats_l1::dataset::Cell::Number(v) => *v,
        other => panic!("{other:?}"),
    };
    let report = validate(&d, &spec(OperationId(1), &d, params(1, &[5], vec![score_value])));
    let got = codes(&report.errors);
    let text = got.iter().position(|c| *c == C::ColumnHoldsText.wire()).expect("the outcome's text");
    let score = got.iter().position(|c| *c == C::CodeEqualsScore.wire()).expect("the score's code");
    assert!(text < score, "{got:?}");
}

/// The data of decision 698's two cases (the review of 9e4b7ad9, finding 5), i = 1 to
/// 100,000, by the same integer formulas as make_linear_regression_sd_exact.R: u_i = (7919 i)
/// mod 1000 times 2^-527, whose sum of squares about its mean is 1.94 times the smallest
/// normal double, so its variance is subnormal; w_i = (7907 i) mod 1013; y_i = (104729 i) mod
/// 997. Every value is an exact double.
fn subnormal_variance_columns() -> [Vec<f64>; 3] {
    let n = 100_000u64;
    let small = (1..=n).map(|i| ((7919 * i) % 1000) as f64 * 2f64.powi(-527)).collect();
    let w = (1..=n).map(|i| ((7907 * i) % 1013) as f64).collect();
    let y = (1..=n).map(|i| ((104729 * i) % 997) as f64).collect();
    [y, w, small]
}

/// gmp's exact values (make_linear_regression_sd_exact.R, 11 Nov 2026), each the double
/// nearest the exact rational: the small column's SD, and its VIF beside w.
const SD_SMALL_EXACT: f64 = 6.5705786229769822e-157;
const VIF_SMALL_EXACT: f64 = 1.000000077691136;

/// Decision 698's gate for the two cases below: ten times the larger error measured over
/// them, 3.597e-14 (the VIF; 2.383e-14 for the SD; macOS aarch64, 11 Nov 2026), rounded up.
/// 9A-1's root of the variance errs by 1.131e-12 on the SD, about three times the gate.
const SUBNORMAL_VARIANCE_GATE: f64 = 4e-13;

/// Decision 698 (finding 5): a model column whose variance is subnormal takes its SD from
/// sd_from's scaled sum, not the root of that variance (9A-1's form), which has lost digits.
#[test]
fn a_model_columns_sd_with_a_subnormal_variance_comes_from_its_scaled_sum() {
    let [y, _, small] = subnormal_variance_columns();
    let rows: Vec<Vec<String>> = (0..y.len()).map(|r| vec![format!("{:?}", y[r]), format!("{:?}", small[r])]).collect();
    let r = compute(&dataset(&["y", "x"], &rows), &params(1, &[2], vec![])).unwrap();
    let m = small.iter().sum::<f64>() / small.len() as f64;
    let variance = small.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (small.len() as f64 - 1.0);
    assert!(variance > 0.0 && variance < f64::MIN_POSITIVE, "the premise: the variance {variance:e} is subnormal");
    let got = v(r.columns[1].sd);
    let error = ((got - SD_SMALL_EXACT) / SD_SMALL_EXACT).abs();
    let old = ((variance.sqrt() - SD_SMALL_EXACT) / SD_SMALL_EXACT).abs();
    println!("the small column's SD: this form {error:e}, 9A-1's root of the variance {old:e}, against gmp");
    assert!(error <= SUBNORMAL_VARIANCE_GATE, "{error:e} over {SUBNORMAL_VARIANCE_GATE:e}");
}

/// Decision 698 (finding 5): a predictor whose variance is subnormal, placed last, takes its
/// VIF from its SD so computed, (sd √s_j)² (n − 1), not from (n − 1) var s_j (9A-1's form).
#[test]
fn a_vif_with_a_subnormal_variance_comes_from_the_scaled_sd() {
    let [y, w, small] = subnormal_variance_columns();
    let rows: Vec<Vec<String>> =
        (0..y.len()).map(|r| vec![format!("{:?}", y[r]), format!("{:?}", w[r]), format!("{:?}", small[r])]).collect();
    let r = compute(&dataset(&["y", "w", "x"], &rows), &params(1, &[2, 3], vec![])).unwrap();
    let got = v(r.predictors[1].vif);
    let error = ((got - VIF_SMALL_EXACT) / VIF_SMALL_EXACT).abs();
    println!("the small predictor's VIF: {got:?}, error {error:e} against gmp");
    assert!(matches!(r.predictors[1].tolerance, Stat::Value(_)));
    assert!(error <= SUBNORMAL_VARIANCE_GATE, "{error:e} over {SUBNORMAL_VARIANCE_GATE:e}");
}
