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
    assert_eq!(r.fit.df, 0);
    assert!(val(r.error_variances[0].estimate) < 0.0, "the premise: θ11 below 0");
    assert_eq!(r.notes, vec![C::NegativeVariance, C::DfZero]);
    // An SMC above 1 where θ is below 0, as decision 745 gives it.
    assert!(val(r.smc[0].value) > 1.0);
    let f = &r.fit;
    for s in [f.p, f.cmin_df, f.agfi, f.pgfi, f.rfi, f.tli, f.pratio, f.pnfi, f.pcfi, f.ncp_lo90, f.ncp_hi90, f.f0_lo90, f.f0_hi90, f.rmsea, f.rmsea_lo90, f.rmsea_hi90, f.pclose, f.hoelter05, f.hoelter01, f.ecvi_lo90, f.ecvi_hi90] {
        assert_eq!(code(s), C::DfZero.wire());
    }
    // At 0 degrees of freedom the minimum is rounding of either sign; below 0 it is
    // clamped to 0, above it stays (6.4e-12 here).
    assert!(val(f.cmin) >= 0.0 && val(f.cmin) < 1e-9, "{}", val(f.cmin));
}

#[test]
fn bcc_and_mecvi_need_more_than_p_plus_2_rows() {
    let mut cols = vec![vec![1.0, 2.0, 3.0, 4.0, 6.0], vec![2.0, 1.0, 4.0, 3.0, 5.0], vec![1.0, 3.0, 2.0, 5.0, 4.0]];
    let r = compute(&table(&cols), &model(&[&[1, 2, 3]], true)).unwrap();
    assert_eq!(r.rows_used, 5);
    assert_eq!(code(r.fit.bcc), C::TooFewRowsForBcc.wire());
    assert_eq!(code(r.fit.mecvi), C::TooFewRowsForBcc.wire());
    for (c, x) in cols.iter_mut().zip([5.0, 6.0, 6.0]) {
        c.push(x);
    }
    let r = compute(&table(&cols), &model(&[&[1, 2, 3]], true)).unwrap();
    assert_eq!(r.rows_used, 6);
    assert!(matches!(r.fit.bcc, Stat::Value(_)) && matches!(r.fit.mecvi, Stat::Value(_)));
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
    worst.0 = worst.0.max(relative(val(back.fit.cmin), val(base.fit.cmin)));
    worst.1 = worst.1.max(relative(val(big.fit.cmin), val(base.fit.cmin)));
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
    // Capped one short of the steps it needs, it refuses with the cap as its limit.
    let cap = (r.iterations - 1) as u32;
    let e = compute_with_cap(&d, &p, cap).unwrap_err();
    assert_eq!(e.code, C::CfaNotConverged);
    assert_eq!(e.params.get(&ParamKey::Limit), Some(&ParamValue::Int(i64::from(cap))));
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
    let mut f = keys(&v["fit"]);
    f.sort();
    assert_eq!(f.len(), 41);
    for k in ["npar", "cmin", "df", "p", "cminDf", "rmr", "gfi", "agfi", "pgfi", "nfi", "rfi", "ifi", "tli", "cfi", "pratio", "pnfi", "pcfi", "ncp", "ncpLo90", "ncpHi90", "fmin", "f0", "f0Lo90", "f0Hi90", "rmsea", "rmseaLo90", "rmseaHi90", "pclose", "aic", "bcc", "bic", "caic", "ecvi", "ecviLo90", "ecviHi90", "mecvi", "hoelter05", "hoelter01", "independenceNpar", "independenceCmin", "independenceDf"] {
        assert!(f.iter().any(|x| x == k), "fit has no {k}");
    }
    assert!(v["fit"]["npar"].is_u64() && v["fit"]["df"].is_u64() && v["iterations"].is_u64());
    assert_eq!(v["fit"]["rmsea"].as_object().unwrap().keys().next().unwrap(), "value");
}

/* ---- the check points --------------------------------------------------------- */

#[test]
fn the_check_points_come_in_their_order_and_the_iterations_repeat_their_own() {
    let d = table(&structured(200, 6, 2));
    let p = structured_model(6, 2);
    assert_eq!(check_points(&p), 1 + 6 + 2);
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
    // A stop mid-covariances and one during the iterations.
    for (at, nth) in [(3usize, 1usize), (7, 2)] {
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
    assert!(val(misfit.fit.cmin) > 100.0 * val(fits.fit.cmin).max(1.0), "the premise: the second model fits worse");
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
    let (c, d) = (val(r.fit.cmin), r.fit.df as f64);
    let (cb, db) = (val(r.fit.independence_cmin), r.fit.independence_df as f64);
    assert!(c - d > cb - db && cb - db > 0.0, "the premise: {} > {} > 0", c - d, cb - db);
    assert!(r.notes.is_empty(), "a proper solution");
    assert_eq!(val(r.fit.cfi), 0.0);
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
    assert_eq!(r.fit.df, 0);
    assert_eq!(r.notes, vec![C::DfZero]);
    assert_eq!(val(r.fit.cmin).to_bits(), 0.0f64.to_bits(), "{:e}", val(r.fit.cmin));
}
