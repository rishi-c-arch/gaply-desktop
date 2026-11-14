//! **`gaply_core::stats_cfa`'s cases (Research OS, Phase 9C-1, decisions 745
//! and 746)**, beside the R golden and the AMOS tier: every refusal, exclusion
//! and note by code and in its order; the not-computable paths the golden does
//! not reach; permuting the rows; multiplying every value by 1,000; the Newton
//! decrement's stop and the cap's refusal; the same bits on repeated runs; the
//! result's exact keys; the check points; and the widest result's size.

use std::ops::ControlFlow;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ParamKey, ParamValue};
use gaply_core::stats_cfa::{
    check_points, compute, compute_with_cap, run, spec, validate, CfaCode as C, CfaOutcome, CfaParams, Halt, Stat,
    DECREMENT_STOP, MAX_FACTORS, MAX_ITEMS,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};

fn dataset(headers: &[String], rows: &[Vec<String>]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers.to_vec(), rows.to_vec()),
        DataFingerprint::of_bytes(format!("{headers:?}{}{rows:?}", rows.len()).as_bytes(), "cfa cases", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn table(columns: &[Vec<f64>]) -> Dataset {
    let headers: Vec<String> = (1..=columns.len()).map(|j| format!("x{j}")).collect();
    let rows: Vec<Vec<String>> = (0..columns[0].len()).map(|r| columns.iter().map(|c| format!("{:?}", c[r])).collect()).collect();
    dataset(&headers, &rows)
}

fn a(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn model(factors: &[&[u32]], correlated: bool) -> CfaParams {
    CfaParams {
        indicators: factors.iter().map(|f| f.iter().map(|c| a(*c)).collect()).collect(),
        correlated_factors: correlated,
        missing_codes: Vec::new(),
    }
}

fn codes(errors: &[gaply_core::contracts::Diagnostic<C>]) -> Vec<&'static str> {
    errors.iter().map(|e| e.code.wire()).collect()
}

fn val(s: Stat) -> f64 {
    match s {
        Stat::Value(x) => x,
        Stat::NotComputable(c) => panic!("not computable: {}", c.wire()),
    }
}

fn code(s: Stat) -> &'static str {
    match s {
        Stat::Value(x) => panic!("a value: {x}"),
        Stat::NotComputable(c) => c.wire(),
    }
}

/// K factors over n rows and p items, every value an integer: item j on factor
/// (j − 1) mod K, each factor's score its own multiple of the row, a term every
/// factor shares (cfa_r_golden.rs's formula).
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

/// The factors of `structured`: factor f's items f, f + k, f + 2k, ….
fn structured_model(p: u32, k: u32) -> CfaParams {
    CfaParams {
        indicators: (1..=k).map(|f| (f..=p).step_by(k as usize).map(a).collect()).collect(),
        correlated_factors: true,
        missing_codes: Vec::new(),
    }
}

/* ---- the checks ------------------------------------------------------------ */

#[test]
fn the_checks_come_in_decision_745s_order_and_validate_lists_every_one() {
    let cols = structured(40, 6, 2);
    let mut rows: Vec<Vec<String>> = (0..40).map(|r| cols.iter().map(|c| format!("{:?}", c[r])).collect()).collect();
    rows[3][1] = "abc".into();
    rows[5][2] = "inf".into();
    let headers: Vec<String> = (1..=6).map(|j| format!("x{j}")).collect();
    let d = dataset(&headers, &rows);
    // Codes; a factor of one item; items out of order in a factor; the factors
    // out of order; a repeat across factors; text; a non-finite value; an
    // unknown column.
    let p = CfaParams {
        indicators: vec![vec![a(4), a(2)], vec![a(1)], vec![a(3), a(4), a(9)]],
        correlated_factors: true,
        missing_codes: vec![f64::NAN, 2.0, 2.0, 1.0],
    };
    let got = codes(&validate(&d, &spec(OperationId(1), &d, p)).errors);
    assert_eq!(
        got,
        vec![
            C::MissingCodeNotFinite.wire(),
            C::MissingCodeRepeated.wire(),
            C::MissingCodesOutOfOrder.wire(),
            C::FactorTooFewItems.wire(),
            C::ItemsOutOfOrder.wire(),
            C::FactorsOutOfOrder.wire(),
            C::ItemRepeated.wire(),
            C::ColumnHoldsText.wire(),
            C::ColumnNonFinite.wire(),
            C::UnknownColumn.wire(),
        ]
    );
    // The factor too few items names its number; factors out of order name the
    // later factor's.
    let clean = table(&structured(40, 6, 2));
    let d = &clean;
    let r = validate(d, &spec(OperationId(1), d, model(&[&[1, 2], &[3]], true)));
    assert_eq!(codes(&r.errors), vec![C::FactorTooFewItems.wire()]);
    assert_eq!(r.errors[0].params.get(&ParamKey::Index), Some(&ParamValue::Int(2)));
    assert_eq!(r.errors[0].params.get(&ParamKey::Limit), Some(&ParamValue::Int(2)));
    let r = validate(d, &spec(OperationId(1), d, model(&[&[3, 4], &[1, 2]], true)));
    assert_eq!(codes(&r.errors), vec![C::FactorsOutOfOrder.wire()]);
    assert_eq!(r.errors[0].params.get(&ParamKey::Index), Some(&ParamValue::Int(2)));
    // A repeat within one factor.
    let r = validate(d, &spec(OperationId(1), d, model(&[&[1, 1, 2]], true)));
    assert_eq!(codes(&r.errors), vec![C::ItemRepeated.wire()]);
    // No factors; too many; too few items in all.
    assert_eq!(codes(&validate(&clean, &spec(OperationId(1), &clean, model(&[], true))).errors), vec![C::NoFactors.wire(), C::TooFewItems.wire()]);
    let r = validate(&clean, &spec(OperationId(1), &clean, model(&[&[1, 2]], true)));
    assert_eq!(codes(&r.errors), vec![C::TooFewItems.wire()]);
    assert_eq!(r.errors[0].params.get(&ParamKey::Limit), Some(&ParamValue::Int(3)));
    let wide = table(&structured(20, MAX_ITEMS as u64, 17));
    let seventeen = structured_model(MAX_ITEMS as u32, 17);
    let r = validate(&wide, &spec(OperationId(1), &wide, seventeen));
    assert_eq!(codes(&r.errors), vec![C::TooManyFactors.wire()]);
    assert_eq!(r.errors[0].params.get(&ParamKey::Count), Some(&ParamValue::Int(17)));
    assert_eq!(r.errors[0].params.get(&ParamKey::Limit), Some(&ParamValue::Int(MAX_FACTORS as i64)));
    // A table holds at most 128 columns, so 129 items name one twice.
    let mut many = structured_model(MAX_ITEMS as u32, 16);
    many.indicators[15].push(a(MAX_ITEMS as u32));
    let r = validate(&wide, &spec(OperationId(1), &wide, many));
    assert_eq!(codes(&r.errors), vec![C::TooManyItems.wire(), C::ItemRepeated.wire()]);
    assert_eq!(r.errors[0].params.get(&ParamKey::Count), Some(&ParamValue::Int(129)));
}

#[test]
fn the_rows_checks_come_only_with_no_other_error_each_only_when_none_before_it_applies() {
    let three = model(&[&[1, 2, 3]], true);
    // Every row has a hole: no complete rows.
    let holes = dataset(&["x1".into(), "x2".into(), "x3".into()], &[vec!["1".into(), "".into(), "3".into()], vec!["".into(), "2".into(), "3".into()]]);
    assert_eq!(compute(&holes, &three).unwrap_err().code, C::NoCompleteRows);
    // Three complete rows for three items: fewer than p + 1.
    let few = table(&[vec![1.0, 2.0, 4.0], vec![2.0, 1.0, 5.0], vec![3.0, 7.0, 1.0]]);
    let e = compute(&few, &three).unwrap_err();
    assert_eq!(e.code, C::TooFewRows);
    assert_eq!(e.params.get(&ParamKey::Count), Some(&ParamValue::Int(3)));
    assert_eq!(e.params.get(&ParamKey::Limit), Some(&ParamValue::Int(4)));
    // A constant item.
    let constant = table(&[vec![1.0, 2.0, 4.0, 5.0, 3.0], vec![2.0, 2.0, 2.0, 2.0, 2.0], vec![3.0, 7.0, 1.0, 2.0, 9.0]]);
    let e = compute(&constant, &three).unwrap_err();
    assert_eq!(e.code, C::ItemConstant);
    assert_eq!(e.params.get(&ParamKey::Column), Some(&ParamValue::Int(2)));
}

/* ---- rows, exclusions and notes --------------------------------------------- */

#[test]
fn a_row_left_out_counts_once_under_its_first_column_in_the_datasets_order() {
    let mut cols = structured(60, 6, 2);
    // Factor 1 is columns 1, 3, 5 and factor 2 is 2, 4, 6, so the spec's order (1,
    // 3, 5, 2, 4, 6) is not the dataset's. Row 3: a code in column 4, a hole in
    // column 5: counted under 4. Row 7: a hole in column 2 only. Row 9: codes in
    // columns 1 and 4 and a hole in column 6: under 1.
    cols[3][2] = -99.0;
    cols[3][8] = -99.0;
    cols[0][8] = -99.0;
    let mut rows: Vec<Vec<String>> = (0..60).map(|r| cols.iter().map(|c| format!("{:?}", c[r])).collect()).collect();
    rows[2][4] = String::new();
    rows[6][1] = String::new();
    rows[8][5] = String::new();
    let headers: Vec<String> = (1..=6).map(|j| format!("x{j}")).collect();
    let d = dataset(&headers, &rows);
    let mut p = model(&[&[1, 3, 5], &[2, 4, 6]], true);
    p.missing_codes = vec![-99.0];
    let r = compute(&d, &p).unwrap();
    assert_eq!(r.rows_used, 57);
    let got: Vec<(&str, i64, u64)> = r
        .exclusions
        .iter()
        .map(|e| {
            let ParamValue::Int(c) = e.params[&ParamKey::Column] else { panic!() };
            (e.code.wire(), c, e.count)
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (C::MissingCode.wire(), 1, 1),
            (C::MissingValue.wire(), 2, 1),
            (C::MissingCode.wire(), 4, 1),
        ]
    );
}

#[test]
fn a_heywood_case_at_df_0_gives_both_notes_in_order_and_df_zeros_codes() {
    // One factor of three columns, the first's loading squared above its
    // variance: x1 = u + w / 10, x2 = u + v, x3 = u − v with v nearly as large as
    // u, so r12 r13 / r23 is about 5.
    let n = 300u64;
    let wave = |i: u64, m: u64| ((i * m) % 1013) as f64 - 506.0;
    let mut cols = vec![Vec::new(); 3];
    for i in 1..=n {
        let (u, v, w) = (wave(i, 7907), 0.894 * wave(i, 104729), wave(i, 15485863));
        cols[0].push(u + 0.1 * w);
        cols[1].push(u + v);
        cols[2].push(u - v);
    }
    let r = compute(&table(&cols), &model(&[&[1, 2, 3]], true)).unwrap();
    assert_eq!(r.fit.model.df, 0);
    assert!(val(r.error_variances[0].estimate) < 0.0, "the premise: θ11 below 0");
    assert_eq!(r.notes, vec![C::NegativeVariance, C::DfZero]);
    // An SMC above 1 where θ is below 0, as decision 745 gives it.
    assert!(val(r.smc[0].value) > 1.0);
    let f = &r.fit.model;
    for s in [f.p, f.cmin_df, f.agfi, f.pgfi, f.rfi, f.tli, f.pratio, f.pnfi, f.pcfi, f.ncp_lo90, f.ncp_hi90, f.f0_lo90, f.f0_hi90, f.rmsea, f.rmsea_lo90, f.rmsea_hi90, f.pclose, f.hoelter05, f.hoelter01, f.ecvi_lo90, f.ecvi_hi90] {
        assert_eq!(code(s), C::DfZero.wire());
    }
    // At 0 degrees of freedom the minimum is rounding of either sign, and an F of
    // at most 1e-12 is written as 0 (decision 761): C was 6.37e-12 here before
    // that rule (F 2.1e-14), and is 0 now.
    assert_eq!(val(f.cmin).to_bits(), 0.0f64.to_bits(), "{}", val(f.cmin));
}

#[test]
fn bcc_and_mecvi_need_more_than_p_plus_2_rows() {
    let mut cols = vec![vec![1.0, 2.0, 3.0, 4.0, 6.0], vec![2.0, 1.0, 4.0, 3.0, 5.0], vec![1.0, 3.0, 2.0, 5.0, 4.0]];
    let r = compute(&table(&cols), &model(&[&[1, 2, 3]], true)).unwrap();
    assert_eq!(r.rows_used, 5);
    assert_eq!(code(r.fit.model.bcc), C::TooFewRowsForBcc.wire());
    assert_eq!(code(r.fit.model.mecvi), C::TooFewRowsForBcc.wire());
    for (c, x) in cols.iter_mut().zip([5.0, 6.0, 6.0]) {
        c.push(x);
    }
    let r = compute(&table(&cols), &model(&[&[1, 2, 3]], true)).unwrap();
    assert_eq!(r.rows_used, 6);
    assert!(matches!(r.fit.model.bcc, Stat::Value(_)) && matches!(r.fit.model.mecvi, Stat::Value(_)));
}

#[test]
fn the_fixed_loadings_have_no_standard_error_cr_or_p() {
    let r = compute(&table(&structured(200, 6, 2)), &structured_model(6, 2)).unwrap();
    for (i, l) in r.loadings.iter().enumerate() {
        let marker = i == 0 || i == 3;
        if marker {
            assert_eq!(val(l.estimate), 1.0);
            for s in [l.se, l.cr, l.p] {
                assert_eq!(code(s), C::FixedParameter.wire());
            }
        } else {
            assert!(val(l.se) > 0.0);
        }
    }
    assert_eq!(r.limitations.iter().map(|c| c.wire()).collect::<Vec<_>>(), [
        "stats.cfa.ml_only",
        "stats.cfa.no_means",
        "stats.cfa.one_group_only",
        "stats.cfa.simple_structure_only",
        "stats.cfa.marker_identification_only",
        "stats.cfa.no_modification_indices",
        "stats.cfa.normal_theory_standard_errors",
        "stats.cfa.no_srmr",
        "stats.cfa.listwise_only",
        "stats.cfa.no_weights",
    ]);
}

/* ---- invariance ------------------------------------------------------------- */

fn relative(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        a.abs()
    } else {
        (a - b).abs() / b.abs()
    }
}

#[test]
fn permuting_the_rows_or_multiplying_every_value_by_1000_moves_nothing_beyond_the_gates() {
    let cols = structured(300, 9, 3);
    let p = structured_model(9, 3);
    let base = compute(&table(&cols), &p).unwrap();
    let reversed: Vec<Vec<f64>> = cols.iter().map(|c| c.iter().rev().copied().collect()).collect();
    let back = compute(&table(&reversed), &p).unwrap();
    let scaled: Vec<Vec<f64>> = cols.iter().map(|c| c.iter().map(|x| x * 1000.0).collect()).collect();
    let big = compute(&table(&scaled), &p).unwrap();
    let mut worst = (0.0f64, 0.0f64);
    for (i, l) in base.loadings.iter().enumerate() {
        worst.0 = worst.0.max(relative(val(back.loadings[i].estimate), val(l.estimate)));
        worst.1 = worst.1.max(relative(val(big.loadings[i].standardized), val(l.standardized)));
        worst.1 = worst.1.max(relative(val(big.smc[i].value), val(base.smc[i].value)));
        // The loadings themselves do not change with the units: both columns move.
        worst.1 = worst.1.max(relative(val(big.loadings[i].estimate), val(l.estimate)));
    }
    for (c, x) in base.factor_covariances.iter().zip(&big.factor_covariances) {
        worst.1 = worst.1.max(relative(val(x.correlation), val(c.correlation)));
    }
    worst.0 = worst.0.max(relative(val(back.fit.model.cmin), val(base.fit.model.cmin)));
    worst.1 = worst.1.max(relative(val(big.fit.model.cmin), val(base.fit.model.cmin)));
    println!("permuted: worst relative {:.3e}; times 1,000: worst relative {:.3e}", worst.0, worst.1);
    // The golden's gates for estimates (2.1e-8) and standardized values (3.3e-8).
    assert!(worst.0 <= 2.1e-8 && worst.1 <= 3.3e-8, "{worst:?}");
    // Variances move by 1e6, exactly as the units.
    assert!(relative(val(big.factor_variances[0].estimate), 1e6 * val(base.factor_variances[0].estimate)) <= 2.1e-8);
}

/* ---- the estimation's stop and cap ------------------------------------------ */

#[test]
fn the_run_stops_at_its_decrement_and_refuses_at_its_cap() {
    let d = table(&structured(300, 9, 3));
    let p = structured_model(9, 3);
    let r = compute(&d, &p).unwrap();
    assert!(r.iterations >= 2, "the premise: more than one step ({})", r.iterations);
    assert!(DECREMENT_STOP == 1e-20);
    // Capped one short of the steps either attempt needs, it refuses with the cap
    // as its limit: decision 745's start takes 10 iterations here and the triad
    // attempt (decision 761) 8 (measured on the Mac, 14 Nov 2026; at a cap of 8
    // or 9 the triad attempt gives CMIN 10.921377857923446 against
    // 10.921377857931944 uncapped), so the cap is 7.
    assert_eq!(r.iterations, 10, "the premise: decision 745's start takes 10");
    let cap = 7u32;
    let e = compute_with_cap(&d, &p, cap).unwrap_err();
    assert_eq!(e.code, C::CfaNotConverged);
    assert_eq!(e.params.get(&ParamKey::Limit), Some(&ParamValue::Int(i64::from(cap))));
    // With the iterations done (decision 762): both attempts reach the cap.
    assert_eq!(e.params.get(&ParamKey::Count), Some(&ParamValue::Int(i64::from(cap))));
    // Capped at 8, the triad attempt converges in its 8 (the review of this step,
    // finding 11: the comment's count, asserted).
    assert_eq!(compute_with_cap(&d, &p, 8).unwrap().iterations, 8);
    // Capped at exactly its steps, it gives the same result, bit for bit.
    let again = compute_with_cap(&d, &p, r.iterations as u32).unwrap();
    assert_eq!(again.canonical_bytes(), r.canonical_bytes());
}

#[test]
fn repeated_runs_give_the_same_bits() {
    let d = table(&structured(300, 12, 4));
    let p = structured_model(12, 4);
    let first = compute(&d, &p).unwrap().canonical_bytes();
    for _ in 0..3 {
        assert_eq!(compute(&d, &p).unwrap().canonical_bytes(), first);
    }
}

/* ---- the wire ---------------------------------------------------------------- */

#[test]
fn the_result_has_exactly_its_keys() {
    let r = compute(&table(&structured(200, 6, 2)), &structured_model(6, 2)).unwrap();
    let v = serde_json::to_value(&r).unwrap();
    let keys = |x: &serde_json::Value| -> Vec<String> { x.as_object().unwrap().keys().cloned().collect() };
    let mut top = keys(&v);
    top.sort();
    assert_eq!(top, ["errorVariances", "exclusions", "factorCovariances", "factorVariances", "fit", "indicators", "items", "iterations", "limitations", "loadings", "notes", "rowsUsed", "smc"]);
    let mut l = keys(&v["loadings"][0]);
    l.sort();
    assert_eq!(l, ["cr", "estimate", "factor", "item", "p", "se", "standardized"]);
    let mut c = keys(&v["factorCovariances"][0]);
    c.sort();
    assert_eq!(c, ["correlation", "cr", "estimate", "factors", "p", "se"]);
    // Decision 763: three rows, each with the same 38 measures.
    let mut rows = keys(&v["fit"]);
    rows.sort();
    assert_eq!(rows, ["independence", "model", "saturated"]);
    // In that order on the wire, the struct's (a Value's keys may come sorted).
    let text = serde_json::to_string(&r.fit).unwrap();
    let at = |k: &str| text.find(&format!("\"{k}\":{{")).unwrap_or_else(|| panic!("no {k}"));
    assert!(text.starts_with("{\"model\":{") && at("model") < at("saturated") && at("saturated") < at("independence"));
    for row in ["model", "saturated", "independence"] {
        let mut f = keys(&v["fit"][row]);
        f.sort();
        assert_eq!(f.len(), 38, "{row}");
        for k in ["npar", "cmin", "df", "p", "cminDf", "rmr", "gfi", "agfi", "pgfi", "nfi", "rfi", "ifi", "tli", "cfi", "pratio", "pnfi", "pcfi", "ncp", "ncpLo90", "ncpHi90", "fmin", "f0", "f0Lo90", "f0Hi90", "rmsea", "rmseaLo90", "rmseaHi90", "pclose", "aic", "bcc", "bic", "caic", "ecvi", "ecviLo90", "ecviHi90", "mecvi", "hoelter05", "hoelter01"] {
            assert!(f.iter().any(|x| x == k), "{row} has no {k}");
        }
        assert!(v["fit"][row]["npar"].is_u64() && v["fit"][row]["df"].is_u64(), "{row}");
    }
    assert!(v["iterations"].is_u64());
    assert_eq!(v["fit"]["model"]["rmsea"].as_object().unwrap().keys().next().unwrap(), "value");
}

/* ---- the check points --------------------------------------------------------- */

#[test]
fn the_check_points_come_in_their_order_and_the_iterations_repeat_their_own() {
    let d = table(&structured(200, 6, 2));
    let p = structured_model(6, 2);
    // Decision 762: 0 the start; 1 to p the columns of S; p + 1 every
    // iteration; p + 2 before the estimates; p + 3 before the default model's
    // fit; p + 4 before the independence model's.
    assert_eq!(check_points(&p), 1 + 6 + 4);
    let mut seen = Vec::new();
    let r = run(&d, &p, |i| {
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
    assert_eq!(distinct, (0..check_points(&p)).collect::<Vec<usize>>());
    let estimation = seen.iter().filter(|i| **i == 7).count() as u64;
    assert_eq!(estimation, 1 + r.iterations, "one at the start and one per step");
    // A stop mid-covariances, one during the iterations, and one before each
    // model's fit (p + 3 and p + 4).
    for (at, nth) in [(3usize, 1usize), (7, 2), (9, 1), (10, 1)] {
        let mut calls = 0;
        let stopped = run(&d, &p, |i| {
            if i == at {
                calls += 1;
                if calls == nth {
                    return ControlFlow::Break("stop");
                }
            }
            ControlFlow::Continue(())
        });
        assert_eq!(stopped.unwrap_err(), Halt::Stopped { reason: "stop" }, "at {at}");
    }
}

/* ---- the result's size --------------------------------------------------------- */

#[test]
fn the_widest_result_is_under_1_mib() {
    let d = table(&structured(600, MAX_ITEMS as u64, MAX_FACTORS as u64));
    let r: CfaOutcome = compute(&d, &structured_model(MAX_ITEMS as u32, MAX_FACTORS as u32)).unwrap();
    let bytes = serde_json::to_vec(&r).unwrap().len();
    println!("the widest result: 128 items, 16 correlated factors, {bytes} bytes");
    assert!(bytes < 1024 * 1024, "{bytes} bytes");
    assert_eq!(r.factor_covariances.len(), 16 * 15 / 2);
}

/* ---- the review of 9C-1 (decision 750) ---------------------------------------- */

/// Each item's fitted variance λ²φ + θ against its sample variance (divisor N),
/// relatively.
fn fitted_variance_gaps(columns: &[Vec<f64>], r: &CfaOutcome) -> Vec<f64> {
    r.loadings
        .iter()
        .map(|l| {
            let Alias(ColumnAlias(k)) = l.item;
            let x = &columns[(k - 1) as usize];
            let n = x.len() as f64;
            let mean = x.iter().sum::<f64>() / n;
            let s = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
            let phi = val(r.factor_variances.iter().find(|v| v.factor == l.factor).unwrap().estimate);
            let theta = val(r.error_variances.iter().find(|e| e.item == l.item).unwrap().estimate);
            let lambda = val(l.estimate);
            ((lambda * lambda * phi + theta) - s).abs() / s
        })
        .collect()
}

/// The premise that makes two mutants of the standardized estimates and the
/// SMCs equivalent (σ̂_ii read as s_ii): at the solution every fitted variance
/// is the sample variance. Measured with R's own scoring refined to 1e-20 over
/// the golden (at most 2.361e-13) and the hunt's 155 refined solutions (at most
/// 1.875e-12). Here on a model that fits and one that does not. The day it
/// fails, standardized estimates and SMCs computed from S would differ from
/// AMOS's, and those mutants stop being equivalent.
#[test]
fn every_fitted_variance_is_the_sample_variance_at_the_solution() {
    let cols = structured(300, 9, 3);
    let fits = compute(&table(&cols), &structured_model(9, 3)).unwrap();
    let misfit = compute(&table(&cols), &model(&[&[1, 2, 3], &[4, 5, 6], &[7, 8, 9]], true)).unwrap();
    assert!(val(misfit.fit.model.cmin) > 100.0 * val(fits.fit.model.cmin).max(1.0), "the premise: the second model fits worse");
    for r in [&fits, &misfit] {
        let worst = fitted_variance_gaps(&cols, r).into_iter().fold(0.0, f64::max);
        assert!(worst <= 1e-11, "{worst:e}");
    }
}

/// AMOS's CFI, 1 − max(C − d, 0) / max(C_b − d_b, C − d, 0): where the model's
/// excess over its df is above the independence model's and above 0, the
/// denominator is the model's own and CFI is exactly 0. Six independent columns,
/// one factor (seed 9 of the probe's LCG, 60 rows).
#[test]
fn cfi_is_0_when_the_models_excess_is_above_the_independence_models() {
    let mut x = 9u64.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let cols: Vec<Vec<f64>> = (0..6)
        .map(|_| {
            (0..60)
                .map(|_| {
                    x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    ((x >> 33) % 1000) as f64
                })
                .collect()
        })
        .collect();
    let r = compute(&table(&cols), &model(&[&[1, 2, 3, 4, 5, 6]], true)).unwrap();
    let (c, d) = (val(r.fit.model.cmin), r.fit.model.df as f64);
    let (cb, db) = (val(r.fit.independence.cmin), r.fit.independence.df as f64);
    assert!(c - d > cb - db && cb - db > 0.0, "the premise: {} > {} > 0", c - d, cb - db);
    assert!(r.notes.is_empty(), "a proper solution");
    assert_eq!(val(r.fit.model.cfi), 0.0);
}

/// Decision 745's second start: x2 exactly uncorrelated with its marker x1 (±1
/// patterns, exact sums), so its start loading is exactly 0, H fails at the
/// start, and the run starts again from every free loading 1. Without that
/// start the run is refused model_not_identified.
#[test]
fn a_start_loading_of_0_with_h_failing_starts_again_from_loadings_of_1() {
    let w = |k: u32| -> Vec<f64> { (0..32u32).map(|i| if (i >> k) & 1 == 0 { 1.0 } else { -1.0 }).collect() };
    let (pa, pb, pc, pd, pe) = (w(0), w(1), w(2), w(3), w(4));
    let add = |u: &[f64], v: &[f64], s: f64| -> Vec<f64> { u.iter().zip(v).map(|(x, y)| x + s * y).collect() };
    let x1 = add(&pa, &pd, 0.5);
    let x2 = add(&pb, &pc, 0.5);
    let x3 = add(&add(&pa, &pb, 1.0), &pe, 1.0);
    let x4 = add(&add(&pa, &pb, 1.0), &pe, -1.0);
    let cov12: f64 = x1.iter().zip(&x2).map(|(a, b)| a * b).sum::<f64>();
    assert_eq!((x1.iter().sum::<f64>(), cov12), (0.0, 0.0), "the premise: exactly uncorrelated");
    let r = compute(&table(&[x1, x2, x3, x4]), &model(&[&[1, 2], &[3, 4]], true)).unwrap();
    assert_eq!(r.iterations, 1);
    assert_eq!(r.notes, vec![C::NegativeVariance, C::FactorCovarianceNotPositiveDefinite]);
}

/// At 0 degrees of freedom the minimum is 0 up to rounding of either sign, and
/// a minimum below 0 is written as a CMIN of 0 (decision 745). Seed 43 of an
/// LCG, three columns sharing a factor, 40 rows: without the clamp its CMIN is
/// −2.771e-13 (measured over seeds 1 to 60: 2 below 0, 56 exactly 0, 2 above;
/// seed 1, an exact 0, left this test green against the mutation
/// r5_cmin_clamp_gone). The Heywood test above lands at +6.4e-12 and so could not
/// reach the clamp (the review of 9C-1, finding 5).
#[test]
fn a_minimum_rounding_below_0_at_0_degrees_of_freedom_is_a_cmin_of_0() {
    let mut x = 43u64.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut next = || {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((x >> 33) % 1000) as f64
    };
    let f: Vec<f64> = (0..40).map(|_| next()).collect();
    let cols: Vec<Vec<f64>> = (0..3).map(|j| f.iter().map(|v| v * (1.0 + j as f64 * 0.5) + next()).collect()).collect();
    let r = compute(&table(&cols), &model(&[&[1, 2, 3]], true)).unwrap();
    assert_eq!(r.fit.model.df, 0);
    assert_eq!(r.notes, vec![C::DfZero]);
    assert_eq!(val(r.fit.model.cmin).to_bits(), 0.0f64.to_bits(), "{:e}", val(r.fit.model.cmin));
}

/// The clamp test's three columns sharing a factor (an LCG at `seed`), at
/// `rows` rows.
fn lcg_three(rows: usize, seed: u64) -> Vec<Vec<f64>> {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut next = || {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((x >> 33) % 1000) as f64
    };
    let f: Vec<f64> = (0..rows).map(|_| next()).collect();
    (0..3).map(|j| f.iter().map(|v| v * (1.0 + j as f64 * 0.5) + next()).collect()).collect()
}

/// Decision 759's B2: at 0 degrees of freedom C rounded above 0 (7.816e-14
/// here, before the fix) and the independence model's CMIN (2.0457) below its
/// 3 degrees of freedom made CFI's denominator C itself, so CFI was 1 − C / C =
/// 0. A minimum F of at most 1e-12 at d = 0 is now written as 0 (decision
/// 761): C = 0 and CFI, NFI and IFI 1. Seed 253 at 12 rows, found on the Mac by
/// FIND FIRST (12 rows, seeds 1 to 20,000: 49 with an independence CMIN below
/// 3, 10 of them with C above 0; the first, 253, a proper solution).
#[test]
fn a_minimum_rounding_above_0_at_0_degrees_of_freedom_is_a_cmin_of_0_and_a_cfi_of_1() {
    let r = compute(&table(&lcg_three(12, 253)), &model(&[&[1, 2, 3]], true)).unwrap();
    assert_eq!(r.fit.model.df, 0);
    assert_eq!(r.notes, vec![C::DfZero], "the premise: a proper solution");
    assert!(val(r.fit.independence.cmin) < 3.0, "the premise: C_b below d_b ({})", val(r.fit.independence.cmin));
    assert_eq!(val(r.fit.model.cmin).to_bits(), 0.0f64.to_bits(), "{:e}", val(r.fit.model.cmin));
    assert_eq!(val(r.fit.model.cfi), 1.0);
    assert_eq!(val(r.fit.model.nfi), 1.0);
    assert_eq!(val(r.fit.model.ifi), 1.0);
}

/// Decision 761's second attempt: data/uncorrelated_rescued.csv, 157 rows of 27
/// columns drawn from five uncorrelated factors by make_cfa_rescue_fixtures.R,
/// which decision 745's starts refuse (7bafd6df: cfa_not_converged at the
/// 500-iteration cap) and the triad starts solve. Its CMIN within 1e-9
/// relative of lavaan 0.7-3's refined to a decrement of 1e-20 by the golden's
/// own refine, and every loading, factor variance and error variance within
/// the golden's estimate gate (2.1e-8 relative, absolute below 1e-12).
#[test]
fn an_uncorrelated_model_decision_745s_starts_refuse_is_solved_from_the_triad_starts() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_cfa");
    let text = std::fs::read_to_string(format!("{dir}/data/uncorrelated_rescued.csv")).unwrap();
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    let d = dataset(&headers, &rows);
    let expected_text = std::fs::read_to_string(format!("{dir}/uncorrelated_rescued_expected.tsv")).unwrap();
    let factors: Vec<Vec<u32>> = expected_text
        .lines()
        .find_map(|l| l.strip_prefix("# factors "))
        .and_then(|l| l.strip_suffix(" uncorrelated"))
        .unwrap()
        .split(';')
        .map(|f| f.split(',').map(|c| c.parse().unwrap()).collect())
        .collect();
    let refs: Vec<&[u32]> = factors.iter().map(|f| f.as_slice()).collect();
    let want: std::collections::BTreeMap<String, f64> = expected_text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("case\t"))
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f[1].to_string(), f[2].parse().unwrap())
        })
        .collect();
    let r = compute(&d, &model(&refs, false)).unwrap_or_else(|e| panic!("refused {}", e.code.wire()));
    assert_eq!(r.rows_used as f64, want["rows_used"]);
    let cmin = val(r.fit.model.cmin);
    assert!(relative(cmin, want["fit.cmin"]) <= 1e-9, "CMIN {cmin:?} against R's {:?}", want["fit.cmin"]);
    let gate = |a: f64, b: f64| (a - b).abs() / b.abs().max(1e-12);
    let mut compared = 0;
    let mut worst = 0.0f64;
    for (i, l) in r.loadings.iter().enumerate() {
        worst = worst.max(gate(val(l.estimate), want[&format!("loading.{i}.estimate")]));
        compared += 1;
    }
    for v in &r.factor_variances {
        worst = worst.max(gate(val(v.estimate), want[&format!("fvar.{}.estimate", v.factor)]));
        compared += 1;
    }
    for (i, e) in r.error_variances.iter().enumerate() {
        worst = worst.max(gate(val(e.estimate), want[&format!("evar.{i}.estimate")]));
        compared += 1;
    }
    println!("rescued in {} iterations; CMIN {cmin:?}; {compared} estimates, worst relative {worst:.3e}", r.iterations);
    assert_eq!(compared, 27 + 5 + 27);
    assert!(worst <= 2.1e-8, "{worst:e}");
}

/* ---- the model's checks and refusals (decision 762) ---------------------------- */

const TWO_ITEM: &str = "stats.cfa.two_item_factor_uncorrelated";

/// An uncorrelated factor of two columns is a one-factor model of its own on 3
/// moments with 4 parameters, so no data identify it (decision 760): refused
/// when the settings are checked, one error per such factor, naming it. With
/// the factors correlated, or one factor, the rule does not apply.
#[test]
fn an_uncorrelated_factor_of_two_columns_is_refused_with_its_number() {
    let d = table(&structured(40, 6, 2));
    let errors = |factors: &[&[u32]], correlated: bool| validate(&d, &spec(OperationId(1), &d, model(factors, correlated))).errors;
    let e = errors(&[&[1, 2], &[3, 4, 5]], false);
    assert_eq!(codes(&e), vec![TWO_ITEM]);
    assert_eq!(e[0].params.get(&ParamKey::Index), Some(&ParamValue::Int(1)));
    assert_eq!(e[0].params.len(), 1);
    let e = errors(&[&[1, 2], &[3, 4]], false);
    assert_eq!(codes(&e), vec![TWO_ITEM, TWO_ITEM]);
    assert_eq!(e[1].params.get(&ParamKey::Index), Some(&ParamValue::Int(2)));
    assert!(errors(&[&[1, 2], &[3, 4, 5]], true).is_empty());
    assert!(errors(&[&[1, 2], &[3, 4]], true).is_empty());
    assert!(errors(&[&[1, 2, 3]], false).is_empty());
    // After factor_too_few_items's errors, on its pattern.
    let e = errors(&[&[1], &[2, 3], &[4, 5, 6]], false);
    assert_eq!(codes(&e), vec![C::FactorTooFewItems.wire(), TWO_ITEM]);
    assert_eq!(e[1].params.get(&ParamKey::Index), Some(&ParamValue::Int(2)));
    // The run refuses it the same way, before any arithmetic.
    let r = compute(&d, &model(&[&[1, 2], &[3, 4, 5]], false)).unwrap_err();
    assert_eq!(r.code.wire(), TWO_ITEM);
}

/// A column whose squares overflow is refused as overflow, naming it, not as
/// a singular S (decision 762). The variance's branch: a column times 1e300.
/// The covariance's branch: since Σab ≤ max(Σa², Σb²), a covariance can
/// overflow beside two finite variances only by rounding at the edge, near
/// sqrt(MAX / 4) for four rows. A seeded search of 2,000,000 columns moved by a
/// few ulps from there (FIND FIRST, 14 Nov 2026, offdiag_search_9c2.rs) found
/// 82; the first is here: s11 = s22 = 4.4942328371557893e307 and s21 not finite.
#[test]
fn a_column_whose_squares_overflow_is_refused_as_overflow() {
    let mut cols = structured(40, 3, 1);
    cols[2] = cols[0].iter().map(|x| x * 1e300).collect();
    let e = compute(&table(&cols), &model(&[&[1, 2, 3]], true)).unwrap_err();
    assert_eq!(e.code.wire(), "stats.cfa.overflow");
    assert_eq!(e.params.get(&ParamKey::Column), Some(&ParamValue::Int(3)));
    let x1 = vec![6.703903964971299e153, -6.7039039649713e153, 6.703903964971299e153, -6.7039039649712956e153];
    let x2 = vec![6.703903964971298e153, -6.7039039649712956e153, 6.7039039649713e153, -6.7039039649713e153];
    let x3 = vec![1.0, 2.0, 4.0, 3.0];
    let e = compute(&table(&[x1, x2, x3]), &model(&[&[1, 2, 3]], true)).unwrap_err();
    assert_eq!(e.code.wire(), "stats.cfa.overflow");
    assert_eq!(e.params.get(&ParamKey::Column), Some(&ParamValue::Int(2)));
}

/// Decision 762: H failing its rule after the start is not model_not_identified,
/// since the model was identified where the attempt started. The bug hunt's
/// h098 (seed 20260746, ~/gaply-corpus/os/phase9/9c1/hunt/; R's values at 2
/// decimals): 7 rows of four columns on two correlated factors of two columns;
/// decision 745's start and the triad start each fail the rule at iteration 3
/// (measured on the Mac, 14 Nov 2026, with the attempts printed by a probe
/// line removed afterwards). 7bafd6df refused it as model_not_identified; it is
/// cfa_not_converged with its 3 iterations. No dataset of the hunt now fails
/// the rule at the start of every attempt, so no data reach
/// model_not_identified there; the unit test of the refusal's choice holds it.
#[test]
fn h_failing_after_the_start_is_cfa_not_converged_with_its_iterations() {
    let rows = [
        [-35.7, -151.33, 25.9, -50.75],
        [-31.84, -230.53, 25.91, -50.55],
        [-50.03, -21.41, 25.9, -51.88],
        [-62.99, 167.29, 25.9, -53.03],
        [-43.64, 74.4, 25.91, -50.53],
        [-55.69, -298.22, 25.9, -55.21],
        [-43.22, -244.74, 25.91, -54.37],
    ];
    let cols: Vec<Vec<f64>> = (0..4).map(|j| rows.iter().map(|r| r[j]).collect()).collect();
    let e = compute(&table(&cols), &model(&[&[1, 4], &[2, 3]], true)).unwrap_err();
    assert_eq!(e.code, C::CfaNotConverged);
    assert_eq!(e.params.get(&ParamKey::Count), Some(&ParamValue::Int(3)));
    assert_eq!(e.params.get(&ParamKey::Limit), Some(&ParamValue::Int(500)));
}

/* ---- AMOS's three rows (decision 763) ------------------------------------------ */

/// The saturated model: Σ = S, NPAR p (p + 1) / 2, CMIN 0, DF 0, RMR 0 and GFI
/// 1 written, and exactly the 21 cells the default model marks df_zero at d = 0
/// df_zero; the other 17 values, from Appendix C's formulas with C = 0.
#[test]
fn the_saturated_row_is_amos_s() {
    let r = compute(&table(&structured(200, 6, 2)), &structured_model(6, 2)).unwrap();
    let s = &r.fit.saturated;
    let (pstar, n, big_n) = (21.0, 199.0, 200.0f64);
    assert_eq!((s.npar, s.df), (21, 0));
    for (name, x) in [("p", s.p), ("cmin_df", s.cmin_df), ("agfi", s.agfi), ("pgfi", s.pgfi), ("rfi", s.rfi), ("tli", s.tli), ("pratio", s.pratio), ("pnfi", s.pnfi), ("pcfi", s.pcfi), ("ncp_lo90", s.ncp_lo90), ("ncp_hi90", s.ncp_hi90), ("f0_lo90", s.f0_lo90), ("f0_hi90", s.f0_hi90), ("rmsea", s.rmsea), ("rmsea_lo90", s.rmsea_lo90), ("rmsea_hi90", s.rmsea_hi90), ("pclose", s.pclose), ("ecvi_lo90", s.ecvi_lo90), ("ecvi_hi90", s.ecvi_hi90), ("hoelter05", s.hoelter05), ("hoelter01", s.hoelter01)] {
        assert_eq!(code(x), C::DfZero.wire(), "{name}");
    }
    for (name, x, want) in [
        ("cmin", s.cmin, 0.0),
        ("rmr", s.rmr, 0.0),
        ("gfi", s.gfi, 1.0),
        ("nfi", s.nfi, 1.0),
        ("ifi", s.ifi, 1.0),
        ("cfi", s.cfi, 1.0),
        ("ncp", s.ncp, 0.0),
        ("fmin", s.fmin, 0.0),
        ("f0", s.f0, 0.0),
        ("aic", s.aic, 2.0 * pstar),
        ("bcc", s.bcc, 2.0 * pstar * (big_n - 1.0) / (big_n - 6.0 - 2.0)),
        ("bic", s.bic, pstar * big_n.ln()),
        ("caic", s.caic, pstar * (big_n.ln() + 1.0)),
        ("ecvi", s.ecvi, 2.0 * pstar / n),
        ("mecvi", s.mecvi, 2.0 * pstar * (big_n - 1.0) / (big_n - 6.0 - 2.0) / n),
    ] {
        assert_eq!(val(x), want, "{name}");
    }
}

/// The independence model: Σ = diag(S), NPAR p, DF p (p − 1) / 2, CMIN C_b; its
/// NFI, RFI, IFI, TLI and CFI 0, its PRATIO 1 and its PNFI and PCFI 0, written,
/// not computed: at decision 761's seed C_b (2.0457) is below d_b (3), where
/// the formulas give CFI 1 and IFI and TLI −0. Every 0 is +0.
#[test]
fn the_independence_rows_baseline_comparisons_are_written_against_itself() {
    let r = compute(&table(&lcg_three(12, 253)), &model(&[&[1, 2, 3]], true)).unwrap();
    let b = &r.fit.independence;
    assert_eq!((b.npar, b.df), (3, 3));
    assert!(val(b.cmin) < 3.0, "the premise: C_b below d_b");
    for (name, x) in [("nfi", b.nfi), ("rfi", b.rfi), ("ifi", b.ifi), ("tli", b.tli), ("cfi", b.cfi), ("pnfi", b.pnfi), ("pcfi", b.pcfi)] {
        assert_eq!(val(x).to_bits(), 0.0f64.to_bits(), "{name} {:?}", val(x));
    }
    assert_eq!(val(b.pratio), 1.0);
    // Its own CMIN is the default model's baseline; its P and RMSEA are computed.
    assert!(matches!(b.p, Stat::Value(_)) && matches!(b.rmsea, Stat::Value(_)));
}

/// h043 (decision 763; 9C-1's report, section 7.2): the bug hunt's dataset on
/// which Gaply's minimum is below lavaan 0.7-3's, with φ11 below 0. lavaan's
/// start cannot be refined to a decrement of 1e-20 there, so it is not a golden
/// case: Gaply's estimates are written to h043_gaply.tsv under
/// GAPLY_WRITE_CFA_H043 (as the pins are), check_h043.R computes R's F, its
/// gradient and the decrement g'H⁻¹g at them by decisions 743 and 745's
/// formulas, and lavaan's own minimum, into h043_r.tsv, and this test holds
/// Gaply to them: R's F within 1e-12 relative, R's decrement at most 1e-16,
/// and Gaply's F below lavaan's by more than 0.4 (9C-1: 0.8025 against 1.273).
#[test]
fn h043_is_a_lower_minimum_than_lavaans_checked_by_r() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_cfa");
    let text = std::fs::read_to_string(format!("{dir}/data/h043.csv")).unwrap();
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    let d = dataset(&headers, &rows);
    let r = compute(&d, &model(&[&[1, 15], &[2, 9, 10, 14], &[3, 4, 8, 11, 12, 13], &[5, 6, 7]], true)).unwrap();
    assert_eq!(r.rows_used, 107);
    // The estimates' bits and the 102 iterations were measured on macOS aarch64
    // only: the decrement at the solution (9.7e-21) sits beside the 1e-20 stop and
    // F goes through ln, so another libm may take one step more or less (the review
    // of this step, finding 1). Elsewhere the estimates are held within 1e-12.
    let measured_here = cfg!(all(target_os = "macos", target_arch = "aarch64"));
    if measured_here {
        assert_eq!(r.iterations, 102);
    }
    assert!(val(r.factor_variances[0].estimate) < 0.0, "φ11 {}", val(r.factor_variances[0].estimate));
    assert_eq!(r.notes, vec![C::NegativeVariance, C::FactorCovarianceNotPositiveDefinite]);
    // Gaply's θ in R's order: the free loadings, the factor variances, the
    // covariances (f < g), the error variances.
    let mut theta: Vec<f64> = r.loadings.iter().filter(|l| l.se != Stat::NotComputable(C::FixedParameter)).map(|l| val(l.estimate)).collect();
    theta.extend(r.factor_variances.iter().map(|v| val(v.estimate)));
    theta.extend(r.factor_covariances.iter().map(|c| val(c.estimate)));
    theta.extend(r.error_variances.iter().map(|e| val(e.estimate)));
    let f = val(r.fit.model.fmin);
    if std::env::var_os("GAPLY_WRITE_CFA_H043").is_some() {
        let mut out = String::from("# Gaply's estimates of h043 (Research OS, Phase 9C-2, decision 763), in R's theta order, written by cfa_cases.rs under GAPLY_WRITE_CFA_H043.\nkey\tvalue\n");
        for (i, t) in theta.iter().enumerate() {
            out.push_str(&format!("theta.{i}\t{t:?}\n"));
        }
        out.push_str(&format!("gaply_f\t{f:?}\n"));
        std::fs::write(format!("{dir}/h043_gaply.tsv"), out).unwrap();
    }
    let written = std::fs::read_to_string(format!("{dir}/h043_gaply.tsv")).unwrap();
    let pinned: Vec<f64> = written.lines().filter(|l| l.starts_with("theta.")).map(|l| l.split('\t').nth(1).unwrap().parse().unwrap()).collect();
    if measured_here {
        assert_eq!(pinned, theta, "h043_gaply.tsv is not this build's estimates");
    } else {
        assert_eq!(pinned.len(), theta.len());
        for (a, b) in theta.iter().zip(&pinned) {
            assert!(relative(*a, *b) <= 1e-12, "{a:?} against the pinned {b:?}");
        }
    }
    let want: std::collections::BTreeMap<String, f64> = std::fs::read_to_string(format!("{dir}/h043_r.tsv"))
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("key\t"))
        .map(|l| {
            let x: Vec<&str> = l.split('\t').collect();
            (x[0].to_string(), x[1].parse().unwrap())
        })
        .collect();
    println!("h043: Gaply F {f:?}, R's F at Gaply's estimates {:?}, R's decrement {:e}, lavaan's F {:?}", want["r_f"], want["r_decrement"], want["lavaan_f"]);
    assert!(relative(f, want["r_f"]) <= 1e-12, "F {f:?} against R's {:?}", want["r_f"]);
    assert!(want["r_decrement"] <= 1e-16, "R's decrement {:e}", want["r_decrement"]);
    assert!(want["lavaan_f"] - f > 0.4, "lavaan's F {:?}, Gaply's {f:?}", want["lavaan_f"]);
}

/* ---- the review of 9C-2 ---------------------------------------------------------- */

/// The run's own model_not_identified (the review of this step, finding 2): every
/// attempt's H failing its rule at the start. Trial 13 of a seeded search of
/// 20,000 four-column datasets, each column a sum of Walsh ±1 patterns over 32
/// rows with coefficients in {0, ±1, ±0.5}, so every covariance is exact (14 Nov
/// 2026: 1,941 of the 20,000 reach it on two correlated factors of two columns;
/// review_probe_9c2.rs). The refusal carries no count, as no data reach the
/// degrees-of-freedom guard's.
#[test]
fn a_run_whose_every_attempt_fails_h_at_its_start_is_model_not_identified() {
    let cols = vec![
        vec![-0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5],
        vec![1.0, 0.0, -1.0, -2.0, 0.0, -1.0, -2.0, -3.0, 3.0, 2.0, 1.0, 0.0, 2.0, 1.0, 0.0, -1.0, 1.0, 0.0, -1.0, -2.0, 0.0, -1.0, -2.0, -3.0, 3.0, 2.0, 1.0, 0.0, 2.0, 1.0, 0.0, -1.0],
        vec![-1.5, 0.5, -1.5, 0.5, 0.5, 2.5, 0.5, 2.5, -2.5, -0.5, -2.5, -0.5, -0.5, 1.5, -0.5, 1.5, -1.5, 0.5, -1.5, 0.5, 0.5, 2.5, 0.5, 2.5, -2.5, -0.5, -2.5, -0.5, -0.5, 1.5, -0.5, 1.5],
        vec![1.5, -0.5, 2.5, 0.5, -0.5, -2.5, 0.5, -1.5, -0.5, -2.5, 0.5, -1.5, -2.5, -4.5, -1.5, -3.5, 3.5, 1.5, 4.5, 2.5, 1.5, -0.5, 2.5, 0.5, 1.5, -0.5, 2.5, 0.5, -0.5, -2.5, 0.5, -1.5],
    ];
    let e = compute(&table(&cols), &model(&[&[1, 2], &[3, 4]], true)).unwrap_err();
    assert_eq!(e.code, C::ModelNotIdentified);
    assert!(e.params.is_empty(), "{:?}", e.params);
}

/// The independence model's C_b is written as max(C_b, 0), as C is (the review
/// of this step, finding 6): on columns uncorrelated to rounding, n (Σ ln s_ii −
/// ln|S|) can round below 0, and C_b is now the independence row's own CMIN,
/// P, RMSEA and HOELTER. Trial 5800 of the same search, three columns on one
/// factor: C_b was −3.4416913763379853e-15.
#[test]
fn the_independence_models_cmin_is_never_below_0() {
    let cols = vec![
        vec![2.0, 0.0, 2.0, 0.0, 2.0, 0.0, 2.0, 0.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 0.0, -2.0, 0.0, -2.0, 0.0, -2.0, 0.0, -2.0],
        vec![-0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5],
        vec![0.5, 1.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5],
    ];
    let r = compute(&table(&cols), &model(&[&[1, 2, 3]], true)).unwrap();
    let b = &r.fit.independence;
    assert_eq!(val(b.cmin).to_bits(), 0.0f64.to_bits(), "{:e}", val(b.cmin));
    assert!(val(b.fmin) >= 0.0 && val(b.ncp) >= 0.0);
}
