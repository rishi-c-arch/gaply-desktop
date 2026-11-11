//! **`gaply_core::stats_factor`'s cases (Research OS, Phase 9B-1, decisions 700
//! and 701)**, beside the R golden: every refusal, exclusion and note by code and
//! in its order; the not-computable paths on data; an exactly singular item whose
//! Cholesky residue is rounding above 1e-14; permuting the rows; an orthogonal
//! rotation keeping every communality; the eigenvalues summing to p; the sign
//! rule; the same bits on repeated runs; the result's exact keys; and the check
//! points.

use std::ops::ControlFlow;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ParamKey, ParamValue};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_factor::{
    check_points, compute, run, spec, validate, Extraction, FactorCode as C, FactorOutcome, FactorParams, Halt,
    Rotation, Stat, MAX_ITEMS,
};
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};

fn dataset(headers: &[String], rows: &[Vec<String>]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers.to_vec(), rows.to_vec()),
        DataFingerprint::of_bytes(format!("{headers:?}{}", rows.len()).as_bytes(), "factor cases", TableRef::FirstTable),
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

fn params(p: u32, extraction: Extraction, factors: Option<u32>, rotation: Rotation) -> FactorParams {
    FactorParams {
        items: (1..=p).map(a).collect(),
        extraction,
        factors,
        rotation,
        iterations: 25,
        convergence: 0.001,
        missing_codes: Vec::new(),
    }
}

fn v(s: Stat) -> f64 {
    match s {
        Stat::Value(x) => x,
        Stat::NotComputable(c) => panic!("not computable: {}", c.wire()),
    }
}

fn codes(errors: &[gaply_core::contracts::Diagnostic<C>]) -> Vec<&'static str> {
    errors.iter().map(|e| e.code.wire()).collect()
}

/// K factors over n rows and p items, every value an integer: item j on factor
/// (j − 1) mod K (make_factor_fixtures.R's formula_data).
fn structured(n: u64, p: u64, k: u64) -> Vec<Vec<f64>> {
    (1..=p)
        .map(|j| {
            (1..=n)
                .map(|i| {
                    let f = (j - 1) % k;
                    let score = ((i * 7907 + f * 15485863) % 1013) as f64 - 506.0;
                    let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as f64 - 498.0;
                    2.0 * score + noise
                })
                .collect()
        })
        .collect()
}

fn refusal(d: &Dataset, p: &FactorParams) -> gaply_core::contracts::Diagnostic<C> {
    compute(d, p).expect_err("a refusal")
}

/* ---- the checks ------------------------------------------------------------ */

#[test]
fn the_checks_come_in_decision_700s_order_and_validate_lists_every_one() {
    let mut cols = structured(40, 5, 2);
    let d0 = table(&cols);
    // Text in item 2, an infinity in item 3.
    let mut rows: Vec<Vec<String>> = (0..40).map(|r| cols.iter().map(|c| format!("{:?}", c[r])).collect()).collect();
    rows[3][1] = "abc".into();
    rows[5][2] = "inf".into();
    let headers: Vec<String> = (1..=5).map(|j| format!("x{j}")).collect();
    let d = dataset(&headers, &rows);
    let p = FactorParams {
        items: vec![a(2), a(1), a(3), a(3), a(9)],
        extraction: Extraction::PrincipalAxis,
        factors: Some(5),
        rotation: Rotation::None,
        iterations: 0,
        convergence: 0.5,
        missing_codes: vec![f64::NAN, 2.0, 2.0, 1.0],
    };
    let got = codes(&validate(&d, &spec(OperationId(1), &d, p)).errors);
    assert_eq!(
        got,
        vec![
            C::MissingCodeNotFinite.wire(),
            C::MissingCodeRepeated.wire(),
            C::MissingCodesOutOfOrder.wire(),
            C::ItemsOutOfOrder.wire(),
            C::ItemRepeated.wire(),
            C::ColumnHoldsText.wire(),
            C::ColumnNonFinite.wire(),
            C::ColumnNonFinite.wire(),
            C::UnknownColumn.wire(),
            C::FactorsOutOfRange.wire(),
            C::IterationsOutOfRange.wire(),
            C::ConvergenceOutOfRange.wire(),
        ]
    );
    // Too few and too many items, each with its count and limit.
    let few = validate(&d0, &spec(OperationId(1), &d0, params(2, Extraction::PrincipalComponents, None, Rotation::None)));
    assert_eq!(codes(&few.errors), vec![C::TooFewItems.wire()]);
    assert_eq!(few.errors[0].params.get(&ParamKey::Limit), Some(&ParamValue::Int(3)));
    // A table holds at most 128 columns (build_dataset refuses 129, TooWide), so
    // 129 items name a column twice: too_many_items, then the repeat.
    cols = (0..MAX_ITEMS).map(|j| (0..10).map(|r| ((r * 7 + j * 3) % 11) as f64).collect()).collect();
    let wide = table(&cols);
    let mut p = params(MAX_ITEMS as u32, Extraction::PrincipalComponents, None, Rotation::None);
    p.items.push(a(MAX_ITEMS as u32));
    let many = validate(&wide, &spec(OperationId(1), &wide, p));
    assert_eq!(codes(&many.errors), vec![C::TooManyItems.wire(), C::ItemRepeated.wire()]);
    assert_eq!(many.errors[0].params.get(&ParamKey::Count), Some(&ParamValue::Int(129)));
    assert_eq!(many.errors[0].params.get(&ParamKey::Limit), Some(&ParamValue::Int(128)));
}

#[test]
fn the_factors_limit_is_the_items_for_components_and_one_fewer_for_principal_axis() {
    let d = table(&structured(60, 4, 2));
    for (extraction, ok, bad) in [(Extraction::PrincipalComponents, 4, 5), (Extraction::PrincipalAxis, 3, 4)] {
        let report = |m| codes(&validate(&d, &spec(OperationId(1), &d, params(4, extraction, Some(m), Rotation::None))).errors);
        assert!(report(ok).is_empty(), "{extraction:?} {ok}");
        assert_eq!(report(bad), vec![C::FactorsOutOfRange.wire()], "{extraction:?} {bad}");
        assert_eq!(report(0), vec![C::FactorsOutOfRange.wire()], "{extraction:?} 0");
    }
    let mut p = params(4, Extraction::PrincipalAxis, None, Rotation::None);
    for (iterations, convergence, want) in [
        (1000, 1e-12, vec![]),
        (1001, 1e-12, vec![C::IterationsOutOfRange.wire()]),
        (1, 0.1, vec![]),
        (1, 0.11, vec![C::ConvergenceOutOfRange.wire()]),
        (1, 1e-13, vec![C::ConvergenceOutOfRange.wire()]),
        (1, f64::NAN, vec![C::ConvergenceOutOfRange.wire()]),
    ] {
        p.iterations = iterations;
        p.convergence = convergence;
        assert_eq!(codes(&validate(&d, &spec(OperationId(1), &d, p.clone())).errors), want, "{iterations} {convergence}");
    }
}

#[test]
fn the_three_row_checks_report_at_most_one_and_in_their_order() {
    let headers: Vec<String> = (1..=3).map(|j| format!("x{j}")).collect();
    let row = |x: [&str; 3]| x.iter().map(|s| s.to_string()).collect::<Vec<String>>();
    let none = dataset(&headers, &[row(["1", "", "3"]), row(["", "2", "3"])]);
    assert_eq!(refusal(&none, &params(3, Extraction::PrincipalComponents, None, Rotation::None)).code, C::NoCompleteRows);
    let two = dataset(&headers, &[row(["1", "2", "3"]), row(["2", "1", "5"]), row(["", "2", "3"])]);
    let e = refusal(&two, &params(3, Extraction::PrincipalComponents, None, Rotation::None));
    assert_eq!((e.code, e.params.get(&ParamKey::Count)), (C::TooFewRows, Some(&ParamValue::Int(2))));
    let constant = dataset(&headers, &[row(["1", "4", "3"]), row(["2", "4", "5"]), row(["4", "4", "1"]), row(["3", "4", "2"])]);
    let e = refusal(&constant, &params(3, Extraction::PrincipalComponents, None, Rotation::None));
    assert_eq!((e.code, e.params.get(&ParamKey::Column)), (C::ItemConstant, Some(&ParamValue::Int(2))));
}

#[test]
fn a_row_left_out_is_counted_once_under_its_first_item_without_a_value() {
    let cols = structured(50, 4, 2);
    let mut rows: Vec<Vec<String>> = (0..50).map(|r| cols.iter().map(|c| format!("{:?}", c[r])).collect()).collect();
    rows[0][1] = String::new(); // item 2 empty
    rows[0][3] = "-99".into(); // item 4 a code, after item 2: not counted
    rows[1][3] = "-99".into(); // item 4 a code
    rows[2][0] = "-99".into(); // item 1 a code
    rows[3][3] = String::new(); // item 4 empty
    let headers: Vec<String> = (1..=4).map(|j| format!("x{j}")).collect();
    let d = dataset(&headers, &rows);
    let mut p = params(4, Extraction::PrincipalComponents, Some(2), Rotation::None);
    p.missing_codes = vec![-99.0];
    let r = compute(&d, &p).unwrap();
    assert_eq!(r.rows_used, 46);
    let got: Vec<(&str, u64, i64)> = r
        .exclusions
        .iter()
        .map(|e| (e.code.wire(), e.count, match e.params.get(&ParamKey::Column) { Some(ParamValue::Int(n)) => *n, _ => -1 }))
        .collect();
    assert_eq!(
        got,
        vec![
            (C::MissingCode.wire(), 1, 1),
            (C::MissingValue.wire(), 1, 2),
            (C::MissingValue.wire(), 1, 4),
            (C::MissingCode.wire(), 1, 4),
        ]
    );
}

#[test]
fn a_scale_score_may_be_an_item() {
    // A score is any column; the check refuses only a code equal to its value.
    let d = table(&structured(80, 3, 1));
    assert!(compute(&d, &params(3, Extraction::PrincipalComponents, None, Rotation::None)).is_ok());
}

/* ---- the run's own refusals ------------------------------------------------- */

#[test]
fn a_correlation_a_double_cannot_hold_is_refused_naming_the_pairs_first_item() {
    let mut cols = structured(30, 4, 2);
    cols[1] = cols[1].iter().map(|x| x * 1e300).collect();
    let e = refusal(&table(&cols), &params(4, Extraction::PrincipalComponents, Some(1), Rotation::None));
    assert_eq!((e.code, e.params.get(&ParamKey::Column)), (C::CorrelationOutOfRange, Some(&ParamValue::Int(1))));
}

/// Three columns of a Hadamard matrix of order 8: correlations exactly 0, so
/// every eigenvalue is exactly 1 and none is above 1.
fn hadamard() -> Dataset {
    table(&[
        vec![1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0],
        vec![1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0],
        vec![1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0],
    ])
}

#[test]
fn kaisers_rule_with_no_eigenvalue_above_1_is_refused() {
    assert_eq!(refusal(&hadamard(), &params(3, Extraction::PrincipalComponents, None, Rotation::None)).code, C::NoEigenvalueOverOne);
}

/// Item 3 is exactly item 1 plus item 2, in doubles, by a formula whose sum
/// leaves a rounding residue at the Cholesky step: R's L_33² is 4.2e-14 (n
/// 50,000, d 13), 2.3e-14 (100,000, 13) and 2.0e-14 (100,000, 7)
/// (~/gaply-corpus/os/phase9/9b1/singular/). Decision 699's rule refuses each;
/// a threshold of 1e-14 would not.
#[test]
fn an_item_exactly_the_sum_of_two_others_is_refused_even_where_its_residue_is_above_1e_14() {
    for (n, d) in [(50_000u64, 13.0), (100_000, 13.0), (100_000, 7.0)] {
        let x1: Vec<f64> = (1..=n).map(|i| ((i * 7907) % 1013) as f64 / d).collect();
        let x2: Vec<f64> = (1..=n).map(|i| ((i * 104729) % 997) as f64 / d).collect();
        let x4: Vec<f64> = (1..=n).map(|i| ((i * 15485863) % 1009) as f64 / d).collect();
        let x3: Vec<f64> = x1.iter().zip(&x2).map(|(a, b)| a + b).collect();
        let data = table(&[x1, x2, x3, x4]);
        let e = refusal(&data, &params(4, Extraction::PrincipalAxis, Some(1), Rotation::None));
        assert_eq!((e.code, e.params.get(&ParamKey::Column)), (C::CorrelationsSingular, Some(&ParamValue::Int(3))), "n {n} d {d}");
        // Principal components runs, and every statistic of the inverse is singular.
        let r = compute(&data, &params(4, Extraction::PrincipalComponents, Some(2), Rotation::None)).unwrap();
        let singular = Stat::NotComputable(C::CorrelationsSingular);
        assert_eq!((r.determinant.value, r.determinant.log, r.kmo.overall), (singular, singular, singular));
        assert!(r.kmo.items.iter().all(|i| i.msa == singular));
        assert_eq!((r.bartlett.chi_square, r.bartlett.p), (singular, singular));
    }
}

#[test]
fn an_m_th_eigenvalue_of_0_or_below_is_refused() {
    // Principal axis with p − 1 factors on weakly related items: the reduced
    // matrix's smaller eigenvalues are below 0.
    let cols: Vec<Vec<f64>> = (1..=6u64).map(|j| (1..=40u64).map(|i| ((i * 7919 + j * 104729 + i * j * 13) % 101) as f64).collect()).collect();
    let e = refusal(&table(&cols), &params(6, Extraction::PrincipalAxis, Some(5), Rotation::None));
    assert_eq!(e.code, C::ExtractionEigenvalueNotPositive);
}

#[test]
fn principal_axis_that_does_not_converge_is_refused_with_its_limit() {
    let mut p = params(12, Extraction::PrincipalAxis, Some(3), Rotation::None);
    p.iterations = 2;
    p.convergence = 1e-12;
    let e = refusal(&table(&structured(300, 12, 3)), &p);
    assert_eq!((e.code, e.params.get(&ParamKey::Limit)), (C::ExtractionNotConverged, Some(&ParamValue::Int(2))));
}

/// Item 5 is exactly uncorrelated with every other (32 rows twice, +1 then −1,
/// every mean a multiple of 1/64), so the Jacobi method leaves its row of the
/// two largest eigenvectors exactly 0 (decision 700).
fn isolated() -> Dataset {
    let base: Vec<[f64; 4]> = (0..32u64)
        .map(|i| {
            let x1 = ((i * 7 + 3) % 9 + 1) as f64;
            let x3 = ((i * 5 + 1) % 9 + 1) as f64;
            [x1, x1 + ((i * 11) % 3) as f64 - 1.0, x3, x3 + ((i * 13) % 3) as f64 - 1.0]
        })
        .collect();
    let mut cols = vec![Vec::new(); 5];
    for (half, sign) in [(0, 1.0), (1, -1.0)] {
        let _ = half;
        for row in &base {
            for k in 0..4 {
                cols[k].push(row[k]);
            }
            cols[4].push(sign);
        }
    }
    table(&cols)
}

#[test]
fn an_item_whose_extracted_loadings_are_all_0_is_refused_before_a_rotation() {
    let e = refusal(&isolated(), &params(5, Extraction::PrincipalComponents, Some(2), Rotation::Varimax));
    assert_eq!((e.code, e.params.get(&ParamKey::Column)), (C::RotationItemWithoutLoadings, Some(&ParamValue::Int(5))));
    // Without a rotation the same extraction is given, the item's loadings 0.
    let r = compute(&isolated(), &params(5, Extraction::PrincipalComponents, Some(2), Rotation::None)).unwrap();
    assert_eq!(r.extraction.loadings[4], vec![Stat::Value(0.0), Stat::Value(0.0)]);
}

/* ---- notes and not-computable values ---------------------------------------- */

#[test]
fn one_factor_with_a_rotation_asked_is_not_rotated_and_says_so() {
    let r = compute(&table(&structured(100, 6, 2)), &params(6, Extraction::PrincipalComponents, Some(1), Rotation::Promax)).unwrap();
    assert_eq!(r.notes, vec![C::OneFactorNotRotated]);
    assert!(r.rotation.is_none());
}

#[test]
fn tied_eigenvalues_at_the_m_th_place_are_noted_and_kmo_of_no_correlation_is_too_small() {
    let r = compute(&hadamard(), &params(3, Extraction::PrincipalComponents, Some(1), Rotation::Varimax)).unwrap();
    assert_eq!(r.notes, vec![C::OneFactorNotRotated, C::EigenvaluesNotDistinct], "in decision 700's order");
    assert_eq!(r.kmo.overall, Stat::NotComputable(C::TooSmall), "0 / 0");
    assert_eq!((v(r.determinant.value), v(r.determinant.log), v(r.bartlett.chi_square), v(r.bartlett.p)), (1.0, 0.0, 0.0, 1.0));
}

/// make_factor_fixtures.R's Heywood case (seed 1): a communality above 1 in an
/// iteration of principal axis factoring.
#[test]
fn a_communality_above_1_is_noted() {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_factor/data_heywood_pa.csv")).unwrap();
    let mut lines = text.lines();
    let headers: Vec<String> = lines.next().unwrap().split(',').map(String::from).collect();
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(String::from).collect()).collect();
    let r = compute(&dataset(&headers, &rows), &params(4, Extraction::PrincipalAxis, Some(2), Rotation::None)).unwrap();
    assert_eq!(r.notes, vec![C::CommunalityAboveOne]);
}

/// 120 items, each one factor plus a thousandth of noise: every item's residue
/// is about 1e-6, far above decision 699's rule, and the determinant, about
/// 1e-700, is below the smallest normal double while its log is finite.
#[test]
fn a_determinant_below_the_smallest_normal_double_is_too_small_with_its_log() {
    let n = 400u64;
    let f: Vec<f64> = (1..=n).map(|i| ((i * 7907) % 1013) as f64).collect();
    let cols: Vec<Vec<f64>> = (1..=120u64)
        .map(|j| (1..=n).map(|i| f[i as usize - 1] + (((i * 104729 + j * 7919 + i * j * 31) % 997) as f64 - 498.0) * 1e-3).collect())
        .collect();
    let r = compute(&table(&cols), &params(120, Extraction::PrincipalComponents, Some(1), Rotation::None)).unwrap();
    assert_eq!(r.determinant.value, Stat::NotComputable(C::TooSmall));
    let log = v(r.determinant.log);
    assert!(log < f64::MIN_POSITIVE.ln() && log.is_finite(), "{log}");
    assert!(matches!(r.kmo.overall, Stat::Value(_)) && matches!(r.bartlett.p, Stat::Value(_)));
}

/* ---- properties --------------------------------------------------------------- */

/// The tolerance of the property tests: far above rounding, far below any
/// difference a defect makes.
const PROPERTY_GATE: f64 = 1e-10;

#[test]
fn permuting_the_rows_changes_nothing_beyond_rounding() {
    let cols = structured(200, 8, 2);
    let perm: Vec<usize> = (0..200).map(|i| (i * 37 + 11) % 200).collect();
    let shuffled: Vec<Vec<f64>> = cols.iter().map(|c| perm.iter().map(|&i| c[i]).collect()).collect();
    for extraction in [Extraction::PrincipalComponents, Extraction::PrincipalAxis] {
        let p = params(8, extraction, None, Rotation::Promax);
        let x = serde_json::to_value(compute(&table(&cols), &p).unwrap()).unwrap();
        let y = serde_json::to_value(compute(&table(&shuffled), &p).unwrap()).unwrap();
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        numbers(&x, &mut xs);
        numbers(&y, &mut ys);
        assert_eq!(xs.len(), ys.len());
        let worst = xs.iter().zip(&ys).fold(0.0f64, |m, (a, b)| m.max((a - b).abs() / a.abs().max(1.0)));
        assert!(worst <= PROPERTY_GATE, "{extraction:?}: {worst:e}");
    }
}

fn numbers(j: &serde_json::Value, out: &mut Vec<f64>) {
    match j {
        serde_json::Value::Number(n) => out.push(n.as_f64().unwrap()),
        serde_json::Value::Array(a) => a.iter().for_each(|x| numbers(x, out)),
        serde_json::Value::Object(o) => o.values().for_each(|x| numbers(x, out)),
        _ => {}
    }
}

#[test]
fn varimax_keeps_every_communality_and_the_eigenvalues_sum_to_p() {
    for extraction in [Extraction::PrincipalComponents, Extraction::PrincipalAxis] {
        let r = compute(&table(&structured(300, 12, 3)), &params(12, extraction, Some(3), Rotation::Varimax)).unwrap();
        let rot = r.rotation.as_ref().unwrap();
        for (i, c) in r.extraction.communalities.iter().enumerate() {
            let h: f64 = rot.loadings[i].iter().map(|s| v(*s) * v(*s)).sum();
            assert!((h - v(c.extraction)).abs() <= PROPERTY_GATE, "{extraction:?} item {i}: {h} {}", v(c.extraction));
        }
        let total: f64 = r.eigenvalues.iter().map(|e| v(e.total)).sum();
        assert!((total - 12.0).abs() <= PROPERTY_GATE, "{total}");
        assert!((v(r.eigenvalues.last().unwrap().cumulative) - 100.0).abs() <= 1e-9);
    }
}

/// Decision 699's and 700's rule: every factor's loadings sum to 0 or more,
/// unrotated and rotated.
#[test]
fn every_factor_follows_the_sign_rule() {
    for rotation in [Rotation::None, Rotation::Varimax, Rotation::Promax] {
        let r = compute(&table(&structured(300, 12, 3)), &params(12, Extraction::PrincipalAxis, Some(3), rotation)).unwrap();
        let sums = |m: &Vec<Vec<Stat>>| (0..m[0].len()).map(|k| m.iter().map(|row| v(row[k])).sum::<f64>()).collect::<Vec<f64>>();
        assert!(sums(&r.extraction.loadings).iter().all(|s| *s > 0.0), "{rotation:?}");
        if let Some(rot) = &r.rotation {
            assert!(sums(&rot.loadings).iter().all(|s| *s > 0.0), "{rotation:?}");
            // A reversed factor reverses its row and column of Φ together, so Φ
            // stays symmetric and the structure is still the pattern times Φ
            // (the review of 9B-1: a positive diagonal could not tell).
            if let Some(phi) = &rot.factor_correlations {
                let m = phi.len();
                for j in 0..m {
                    for k in 0..m {
                        assert!((v(phi[j][k]) - v(phi[k][j])).abs() <= PROPERTY_GATE, "{rotation:?}: Φ symmetric");
                    }
                }
                let s = rot.structure.as_ref().unwrap();
                for (i, row) in rot.loadings.iter().enumerate() {
                    for k in 0..m {
                        let want: f64 = (0..m).map(|j| v(row[j]) * v(phi[j][k])).sum();
                        assert!((v(s[i][k]) - want).abs() <= PROPERTY_GATE, "{rotation:?}: structure = pattern Φ");
                    }
                }
            }
            // Varimax: the rotated loadings are the extracted loadings times T,
            // T's columns reversed with the loadings'.
            if let Some(t) = &rot.transformation {
                let m = t.len();
                for (i, row) in r.extraction.loadings.iter().enumerate() {
                    for k in 0..m {
                        let want: f64 = (0..m).map(|j| v(row[j]) * v(t[j][k])).sum();
                        assert!((v(rot.loadings[i][k]) - want).abs() <= PROPERTY_GATE, "varimax: loadings = extracted T");
                    }
                }
            }
        }
    }
}

#[test]
fn repeated_runs_give_the_same_bits() {
    let d = table(&structured(250, 10, 2));
    for rotation in [Rotation::Varimax, Rotation::Promax] {
        let p = params(10, Extraction::PrincipalAxis, None, rotation);
        let a = compute(&d, &p).unwrap().canonical_bytes();
        let b = compute(&d, &p).unwrap().canonical_bytes();
        assert_eq!(a, b, "{rotation:?}");
    }
}

#[test]
fn the_result_has_exactly_its_keys() {
    let keys = |j: &serde_json::Value| j.as_object().unwrap().keys().cloned().collect::<Vec<String>>();
    let r: FactorOutcome = compute(&table(&structured(200, 6, 2)), &params(6, Extraction::PrincipalAxis, Some(2), Rotation::Promax)).unwrap();
    let j = serde_json::to_value(&r).unwrap();
    let mut want = vec!["items", "rowsUsed", "descriptives", "determinant", "kmo", "bartlett", "eigenvalues", "extraction", "rotation", "exclusions", "notes", "limitations"];
    want.sort_unstable();
    assert_eq!(keys(&j), want);
    let mut e = vec!["method", "factors", "iterations", "communalities", "sums", "loadings"];
    e.sort_unstable();
    assert_eq!(keys(&j["extraction"]), e);
    let mut rot = vec!["method", "iterations", "loadings", "transformation", "sums", "structure", "factorCorrelations"];
    rot.sort_unstable();
    assert_eq!(keys(&j["rotation"]), rot);
    assert_eq!((j["rotation"]["transformation"].clone(), j["rotation"]["sums"].clone()), (serde_json::Value::Null, serde_json::Value::Null));
    assert_eq!(keys(&j["bartlett"]), vec!["chiSquare", "df", "p"]);
    assert_eq!(keys(&j["determinant"]), vec!["log", "value"]);
    assert_eq!(keys(&j["kmo"]), vec!["items", "overall"]);
    assert_eq!(keys(&j["descriptives"][0]), vec!["item", "mean", "sd"]);
    assert_eq!(keys(&j["eigenvalues"][0]), vec!["cumulative", "percent", "total"]);
    assert_eq!(keys(&j["extraction"]["communalities"][0]), vec!["extraction", "initial", "item"]);
    assert_eq!(j["extraction"]["method"], "principal_axis");
    assert_eq!(j["rotation"]["method"], "promax");
    let limitations: Vec<&str> = r.limitations.iter().map(|c| c.wire()).collect();
    assert_eq!(
        limitations,
        [
            "no_ml_or_other_extraction",
            "no_oblique_other_than_promax",
            "promax_kappa_four",
            "no_factor_scores",
            "no_matrices_shown",
            "pearson_only",
            "listwise_only",
            "no_scree_plot",
            "no_weights",
        ]
        .map(|c| format!("stats.factor.{c}"))
    );
}

/* ---- the check points --------------------------------------------------------- */

/// Decision 700: 1 + p + p(p − 1)/2 + 2 check points, in their order; every
/// principal axis and varimax iteration calls its point again.
#[test]
fn the_check_points_come_in_their_order_and_the_iterations_repeat_their_own() {
    let d = table(&structured(200, 6, 2));
    let p = params(6, Extraction::PrincipalAxis, Some(2), Rotation::Varimax);
    assert_eq!(check_points(&p), 1 + 6 + 15 + 2);
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
    let extraction_calls = seen.iter().filter(|i| **i == 22).count() as u64;
    let rotation_calls = seen.iter().filter(|i| **i == 23).count() as u64;
    assert_eq!(extraction_calls, 1 + r.extraction.iterations, "one before, one per iteration");
    assert_eq!(rotation_calls, 1 + r.rotation.as_ref().unwrap().iterations, "one before, one per iteration");
    // A stop at an iteration's call stops the run there.
    let mut calls = 0;
    let stopped = run(&d, &p, |i| {
        if i == 22 {
            calls += 1;
            if calls == 3 {
                return ControlFlow::Break("stop");
            }
        }
        ControlFlow::Continue(())
    });
    assert_eq!(stopped.unwrap_err(), Halt::Stopped { reason: "stop" });
    // Without a rotation the rotation's point is still called once.
    let none = params(6, Extraction::PrincipalComponents, Some(2), Rotation::None);
    let mut last = 0;
    run(&d, &none, |i| {
        last = i;
        ControlFlow::<()>::Continue(())
    })
    .unwrap();
    assert_eq!(last, check_points(&none) - 1);
}

/* ---- the result's size -------------------------------------------------------- */

/// app's `project_commands::RESULT_READ_LIMIT_BYTES` (project_commands.rs:142), the
/// smallest limit a saved result passes through (FIND FIRST of 9B-1: the run store
/// and its commands hold no byte limit).
const RESULT_READ_LIMIT_BYTES: usize = 4 * 1024 * 1024;

/// The largest result of a shape the spec allows (decision 701): 128 items,
/// `m` factors and promax, every number the widest JSON a double takes (17
/// significant digits, a sign and an exponent) and every item with two exclusion
/// lines. An upper bound on any real result of that shape.
fn widest(m: usize, extraction: Extraction) -> FactorOutcome {
    use gaply_core::contracts::Exclusion;
    use gaply_core::stats_factor::{
        Communality, Determinant, ExtractedFactors, FactorSums, ItemAdequacy, ItemDescriptive, RotatedFactors,
        SamplingAdequacy, Sphericity,
    };
    let p = MAX_ITEMS;
    let x = Stat::Value(-1.2345678901234567e-123);
    // Where a code is reachable, the widest code (51 bytes against 33): the
    // review of 9B-1. Loadings, transformations and factor correlations come
    // from finite eigenvectors and are always values.
    let c = Stat::NotComputable(C::CorrelationsSingular);
    let square = |rows: usize| vec![vec![x; m]; rows];
    let sums = |k: usize| vec![FactorSums { total: x, percent: x, cumulative: x }; k];
    let items: Vec<Alias> = (1..=p as u32).map(a).collect();
    FactorOutcome {
        items: items.clone(),
        rows_used: u64::MAX,
        descriptives: items.iter().map(|i| ItemDescriptive { item: *i, mean: x, sd: x }).collect(),
        determinant: Determinant { value: c, log: c },
        kmo: SamplingAdequacy { overall: c, items: items.iter().map(|i| ItemAdequacy { item: *i, msa: c }).collect() },
        bartlett: Sphericity { chi_square: c, df: u64::MAX, p: c },
        eigenvalues: sums(p),
        extraction: ExtractedFactors {
            method: extraction,
            factors: m as u64,
            iterations: u64::MAX,
            communalities: items.iter().map(|i| Communality { item: *i, initial: x, extraction: x }).collect(),
            sums: sums(m),
            loadings: square(p),
        },
        rotation: Some(RotatedFactors {
            method: Rotation::Promax,
            iterations: u64::MAX,
            loadings: square(p),
            transformation: Some(square(m)),
            sums: Some(sums(m)),
            structure: Some(square(p)),
            factor_correlations: Some(square(m)),
        }),
        exclusions: items
            .iter()
            .flat_map(|i| {
                [C::MissingValue, C::MissingCode].map(|code| Exclusion {
                    code,
                    count: u64::MAX,
                    params: std::collections::BTreeMap::from([(ParamKey::Column, ParamValue::Int(i64::from(i.0 .0)))]),
                })
            })
            .collect(),
        notes: vec![C::CommunalityAboveOne, C::OneFactorNotRotated, C::EigenvaluesNotDistinct],
        limitations: compute(&table(&structured(30, 3, 1)), &params(3, Extraction::PrincipalComponents, Some(1), Rotation::None))
            .unwrap()
            .limitations,
    }
}

#[test]
fn the_largest_results_the_spec_allows_fit_the_saved_results_read_limit() {
    for (m, extraction) in [(128, Extraction::PrincipalComponents), (127, Extraction::PrincipalAxis)] {
        let bytes = serde_json::to_vec(&widest(m, extraction)).unwrap().len();
        println!("128 items, {m} factors, promax, every field filled at its widest: {bytes} bytes ({:.3} MiB)", bytes as f64 / 1048576.0);
        assert!(bytes < RESULT_READ_LIMIT_BYTES, "{bytes} bytes");
    }
    // The bound is an upper bound: a real 128-item result is smaller.
    let real = compute(&table(&structured(400, 128, 8)), &params(128, Extraction::PrincipalComponents, Some(8), Rotation::Promax)).unwrap();
    let real_bytes = serde_json::to_vec(&real).unwrap().len();
    println!("a real run, 128 items, 8 factors, promax: {real_bytes} bytes");
    assert!(real_bytes < serde_json::to_vec(&widest(8, Extraction::PrincipalComponents)).unwrap().len());
}

/* ---- the review of 9B-1's untested paths ---------------------------------------- */

/// validate warns of a suspect code in an item (three cells of 99), naming the
/// item, the value and the count; the code once ticked, no warning.
#[test]
fn validate_warns_of_a_suspect_code_and_not_once_it_is_ticked() {
    let mut cols = structured(60, 3, 1);
    // Item 2 on a 1 to 5 scale, so three cells of 99 stand far outside it.
    cols[1] = (0..60).map(|r| (r % 5 + 1) as f64).collect();
    for r in [3, 17, 41] {
        cols[1][r] = 99.0;
    }
    let d = table(&cols);
    let p = params(3, Extraction::PrincipalComponents, Some(1), Rotation::None);
    let w = validate(&d, &spec(OperationId(1), &d, p.clone())).warnings;
    assert_eq!(codes(&w), vec![C::SuspectMissingCode.wire()], "{w:?}");
    assert_eq!(
        (w[0].params.get(&ParamKey::Column), w[0].params.get(&ParamKey::Value), w[0].params.get(&ParamKey::Count)),
        (Some(&ParamValue::Int(2)), Some(&ParamValue::Int(99)), Some(&ParamValue::Int(3)))
    );
    let mut ticked = p;
    ticked.missing_codes = vec![99.0];
    assert!(validate(&d, &spec(OperationId(1), &d, ticked)).warnings.is_empty());
}

/// An item exactly uncorrelated with every other: its MSA is 0/0, too_small.
#[test]
fn an_items_msa_with_no_correlation_is_too_small() {
    let r = compute(&isolated(), &params(5, Extraction::PrincipalComponents, Some(2), Rotation::None)).unwrap();
    assert_eq!(r.kmo.items[4].msa, Stat::NotComputable(C::TooSmall));
    assert!(matches!(r.kmo.items[0].msa, Stat::Value(_)));
}

/// Decision 700's premise for Bartlett's multiplier: with no more rows than
/// items the correlations are singular, so the Cholesky factor refuses and no
/// multiplier is used.
#[test]
fn no_more_rows_than_items_makes_the_inverse_statistics_singular() {
    let cols = structured(6, 6, 2);
    let r = compute(&table(&cols), &params(6, Extraction::PrincipalComponents, Some(1), Rotation::None)).unwrap();
    let singular = Stat::NotComputable(C::CorrelationsSingular);
    assert_eq!((r.determinant.value, r.kmo.overall, r.bartlett.chi_square, r.bartlett.p), (singular, singular, singular, singular));
}
