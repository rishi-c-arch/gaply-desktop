//! **`gaply_core::stats_xtab`'s closed outcomes, its check point and its
//! refusals (Research OS, Phase 8X-1, decisions 575 and 577).** The numbers
//! are `crosstabs_r_golden.rs`'s; this file pins what is decided rather than
//! computed: every refusal, exclusion and note by code, the exclusions'
//! order, the levels' order, how a code meets a number and never a text, the
//! check point, and the result's exact keys.

use std::collections::BTreeSet;
use std::ops::ControlFlow;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{
    CapabilityId, Diagnostic, OperationId, ParamKey, ParamValue, ValidationStatus, WorkspaceId,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, MissingRule, RawTable, RawVariable,
    TableRef,
};
use gaply_core::stats_xtab::{
    check_points, compute, run, spec, validate, ChiOutcome, CrosstabsParams, Halt, Stat,
    XtabCode, CAPABILITY_ID, CAPABILITY_VERSION, ENGINE_VERSION, EXPECTED_FLOOR,
};

fn raw(rows: &[(&str, &str)]) -> RawTable {
    RawTable::from_rows(
        vec!["r".into(), "c".into()],
        rows.iter()
            .map(|(r, c)| vec![r.to_string(), c.to_string()])
            .collect(),
    )
}

fn build(raw: RawTable, salt: &str) -> Dataset {
    build_dataset(
        raw,
        DataFingerprint::of_bytes(salt.as_bytes(), "xtab cases", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn dataset(rows: &[(&str, &str)]) -> Dataset {
    build(raw(rows), &format!("{rows:?}"))
}

/// `n` copies of each pair, in the order given.
fn counted(pairs: &[(&'static str, &'static str, usize)]) -> Vec<(&'static str, &'static str)> {
    pairs
        .iter()
        .flat_map(|(r, c, n)| std::iter::repeat((*r, *c)).take(*n))
        .collect()
}

const ROWS: Alias = Alias(ColumnAlias(1));
const COLUMNS: Alias = Alias(ColumnAlias(2));

fn params(codes: &[f64]) -> CrosstabsParams {
    CrosstabsParams {
        rows: ROWS,
        columns: COLUMNS,
        missing_codes: codes.to_vec(),
    }
}

fn refusal(d: &Dataset, p: CrosstabsParams) -> Diagnostic<XtabCode> {
    compute(d, &p).expect_err("refused")
}

fn int(e: &Diagnostic<XtabCode>, key: ParamKey) -> Option<i64> {
    match e.params.get(&key) {
        Some(ParamValue::Int(v)) => Some(*v),
        _ => None,
    }
}

fn base() -> Vec<(&'static str, &'static str)> {
    counted(&[("a", "x", 3), ("a", "y", 2), ("b", "x", 1), ("b", "y", 4)])
}

#[test]
fn the_capability_is_stats_analyze_crosstabs_version_1_engine_1() {
    assert_eq!(CAPABILITY_ID, "stats.analyze.crosstabs");
    assert_eq!((CAPABILITY_VERSION, ENGINE_VERSION), (1, 1));
    assert_eq!(EXPECTED_FLOOR, 5);
    assert_eq!(check_points(), 1);
}

#[test]
fn every_code_is_a_stats_xtab_word_and_holds_no_verdict_word() {
    for c in XtabCode::ALL {
        let w = c.wire();
        assert!(w.starts_with("stats.xtab."), "{w}");
        for bad in ["should", "recommend", "valid", "accurate", "significan"] {
            let words: Vec<&str> = w["stats.xtab.".len()..].split('_').collect();
            assert!(
                !words.contains(&bad) && !w.contains("significan"),
                "{w} holds {bad}"
            );
        }
    }
}

#[test]
fn the_missing_codes_are_refused_by_index_when_not_finite_repeated_or_out_of_order() {
    let d = dataset(&base());
    let e = refusal(&d, params(&[f64::NAN]));
    assert_eq!((e.code, int(&e, ParamKey::Index)), (XtabCode::MissingCodeNotFinite, Some(0)));
    let e = refusal(&d, params(&[1.0, f64::INFINITY]));
    assert_eq!((e.code, int(&e, ParamKey::Index)), (XtabCode::MissingCodeNotFinite, Some(1)));
    let e = refusal(&d, params(&[9.0, 1.0]));
    assert_eq!((e.code, int(&e, ParamKey::Index)), (XtabCode::MissingCodesOutOfOrder, Some(1)));
    let e = refusal(&d, params(&[1.0, 9.0, 9.0]));
    assert_eq!((e.code, int(&e, ParamKey::Index)), (XtabCode::MissingCodeRepeated, Some(2)));
    // Refused, never sorted: the codes in order run.
    assert!(compute(&d, &params(&[1.0, 9.0])).is_ok());
}

#[test]
fn the_column_refusals_name_the_column_by_alias() {
    let d = dataset(&base());
    let p = CrosstabsParams {
        rows: Alias(ColumnAlias(7)),
        ..params(&[])
    };
    let e = refusal(&d, p);
    assert_eq!((e.code, int(&e, ParamKey::Column)), (XtabCode::UnknownColumn, Some(7)));
    let p = CrosstabsParams {
        columns: ROWS,
        ..params(&[])
    };
    assert_eq!(refusal(&d, p).code, XtabCode::SameColumn);
}

#[test]
fn a_column_needs_two_to_fifty_levels_among_the_included_rows() {
    // One level in the rows' column.
    let d = dataset(&counted(&[("a", "x", 2), ("a", "y", 2)]));
    let e = refusal(&d, params(&[]));
    assert_eq!(
        (e.code, int(&e, ParamKey::Column), int(&e, ParamKey::Count)),
        (XtabCode::TooFewLevels, Some(1), Some(1))
    );
    // Two levels in the columns' column, one of them a code: one among the
    // included rows, unlike the ANOVA's group, which counts the whole column.
    let d = dataset(&counted(&[("a", "x", 2), ("b", "7", 2), ("b", "x", 1)]));
    let e = refusal(&d, params(&[7.0]));
    assert_eq!(
        (e.code, int(&e, ParamKey::Column), int(&e, ParamKey::Count)),
        (XtabCode::TooFewLevels, Some(2), Some(1))
    );
    // 50 levels run; 51 are refused, with the count and the limit.
    let labels: Vec<String> = (0..51).map(|i| format!("L{i:02}")).collect();
    let rows51: Vec<(&str, &str)> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| (l.as_str(), if i % 2 == 0 { "x" } else { "y" }))
        .collect();
    let e = refusal(&dataset(&rows51), params(&[]));
    assert_eq!(
        (
            e.code,
            int(&e, ParamKey::Column),
            int(&e, ParamKey::Count),
            int(&e, ParamKey::Limit)
        ),
        (XtabCode::TooManyLevels, Some(1), Some(51), Some(50))
    );
    let r = compute(&dataset(&rows51[..50]), &params(&[])).expect("50 levels run");
    assert_eq!((r.row_levels.len(), r.cell_count), (50, 100));
    // The 51st level a code: 50 among the included rows, so it runs.
    let mut coded = rows51[..50].to_vec();
    coded.push(("99", "x"));
    let r = compute(&dataset(&coded), &params(&[99.0])).expect("the 51st level is a code");
    assert_eq!(r.row_levels.len(), 50);
}

#[test]
fn the_levels_are_in_code_point_order_numbers_included() {
    let d = dataset(&counted(&[
        ("2", "b", 1),
        ("10", "B", 2),
        ("-1", "b", 1),
        ("-2", "\u{e9}", 3),
        ("10", "b", 1),
    ]));
    let r = compute(&d, &params(&[])).unwrap();
    let rows: Vec<&str> = r.row_levels.iter().map(|l| l.label.as_str()).collect();
    let cols: Vec<&str> = r.column_levels.iter().map(|l| l.label.as_str()).collect();
    assert_eq!(rows, ["-1", "-2", "10", "2"]);
    assert_eq!(cols, ["B", "b", "\u{e9}"]);
    assert_eq!(
        r.row_levels.iter().map(|l| l.level).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    // Not by count: "-2" has the most rows and is second.
    assert_eq!(r.row_totals, [1, 3, 3, 1]);
}

#[test]
fn a_code_meets_a_number_in_either_column_and_never_a_text() {
    // "99" declared text in the rows' column is a level; 99 as a number in the
    // columns' column is left out as a code.
    let mut t = raw(&counted(&[
        ("99", "x", 2),
        ("a", "x", 1),
        ("a", "99", 3),
        ("99", "y", 1),
        ("a", "y", 2),
    ]));
    t.variables = vec![
        RawVariable {
            declared_text: Some(true),
            ..Default::default()
        },
        RawVariable::default(),
    ];
    let d = build(t, "declared text 99");
    let r = compute(&d, &params(&[99.0])).unwrap();
    let rows: Vec<&str> = r.row_levels.iter().map(|l| l.label.as_str()).collect();
    assert_eq!(rows, ["99", "a"], "the text 99 is a level");
    assert_eq!(r.exclusions.len(), 1);
    let e = &r.exclusions[0];
    assert_eq!((e.code, e.count), (XtabCode::MissingCode, 3));
    assert_eq!(e.params.get(&ParamKey::Column), Some(&ParamValue::Int(2)));
    // A number coded in the rows' column is left out naming the rows' column.
    let d = dataset(&counted(&[("1", "x", 2), ("2", "y", 2), ("-9", "x", 1), ("1", "y", 1), ("2", "x", 1)]));
    let r = compute(&d, &params(&[-9.0])).unwrap();
    assert_eq!(r.exclusions.len(), 1);
    assert_eq!(
        (r.exclusions[0].code, r.exclusions[0].params.get(&ParamKey::Column)),
        (XtabCode::MissingCode, Some(&ParamValue::Int(1)))
    );
}

#[test]
fn an_spss_user_missing_value_is_missing_never_a_code() {
    let mut t = raw(&counted(&[
        ("1", "x", 2),
        ("2", "y", 2),
        ("1", "y", 1),
        ("2", "x", 1),
        ("9", "x", 3),
    ]));
    t.variables = vec![
        RawVariable {
            user_missing: vec![MissingRule::Value(9.0)],
            ..Default::default()
        },
        RawVariable::default(),
    ];
    let d = build(t, "user missing 9");
    let r = compute(&d, &params(&[9.0])).unwrap();
    assert_eq!(r.exclusions.len(), 1);
    assert_eq!(
        (r.exclusions[0].code, r.exclusions[0].count),
        (XtabCode::MissingRow, 3)
    );
}

#[test]
fn an_infinity_written_out_is_a_text_level_and_nan_is_missing() {
    let d = dataset(&counted(&[
        ("Inf", "x", 2),
        ("1", "y", 2),
        ("1", "x", 1),
        ("Inf", "y", 1),
        ("NaN", "x", 4),
        ("-inf", "y", 1),
    ]));
    let r = compute(&d, &params(&[])).unwrap();
    let rows: Vec<&str> = r.row_levels.iter().map(|l| l.label.as_str()).collect();
    assert_eq!(rows, ["-inf", "1", "Inf"]);
    assert_eq!(r.exclusions.len(), 1);
    assert_eq!(
        (r.exclusions[0].code, r.exclusions[0].count),
        (XtabCode::MissingRow, 4)
    );
}

#[test]
fn a_row_is_left_out_once_by_the_first_reason_rows_column_first() {
    let mut rows = counted(&[("a", "x", 3), ("a", "y", 2), ("b", "x", 1), ("b", "y", 4)]);
    rows.extend([
        ("", ""),     // missing_row, not missing_column too
        ("", "5"),    // missing_row, not a code in the columns' column
        ("5", ""),    // missing_code (rows), not missing_column
        ("5", "5"),   // missing_code (rows) once
        ("a", ""),    // missing_column
        ("b", "5"),   // missing_code (columns)
        ("b", "5"),
    ]);
    let d = dataset(&rows);
    let r = compute(&d, &params(&[5.0])).unwrap();
    let got: Vec<(XtabCode, u64, Option<&ParamValue<XtabCode>>)> = r
        .exclusions
        .iter()
        .map(|e| (e.code, e.count, e.params.get(&ParamKey::Column)))
        .collect();
    assert_eq!(
        got,
        [
            (XtabCode::MissingRow, 2, Some(&ParamValue::Int(1))),
            (XtabCode::MissingCode, 2, Some(&ParamValue::Int(1))),
            (XtabCode::MissingColumn, 1, Some(&ParamValue::Int(2))),
            (XtabCode::MissingCode, 2, Some(&ParamValue::Int(2))),
        ]
    );
    assert_eq!(r.n + 7, d.rows() as u64);
    // A reason with no row is not listed.
    let r = compute(&dataset(&base()), &params(&[5.0])).unwrap();
    assert!(r.exclusions.is_empty());
}

#[test]
fn the_note_is_there_when_one_cell_is_just_below_five_and_not_at_five() {
    // n = 101, rows 12 and 89, columns 42 and 59: E11 = 504 / 101 = 4.99.
    let d = dataset(&counted(&[("a", "x", 6), ("a", "y", 6), ("b", "x", 36), ("b", "y", 53)]));
    let r = compute(&d, &params(&[])).unwrap();
    assert_eq!((r.expected_below_five, r.cell_count), (1, 4));
    assert_eq!(r.notes, [XtabCode::ExpectedBelowFive]);
    assert!(r.smallest_expected < 5.0 && r.smallest_expected > 4.99);
    // n = 100, rows 10 and 90, columns 50 and 50: two cells exactly 5.
    let d = dataset(&counted(&[("a", "x", 5), ("a", "y", 5), ("b", "x", 45), ("b", "y", 45)]));
    let r = compute(&d, &params(&[])).unwrap();
    assert_eq!(r.expected_below_five, 0);
    assert!(r.notes.is_empty());
    assert_eq!(r.smallest_expected, 5.0);
}

#[test]
fn a_two_by_two_table_has_yates_and_fisher_and_a_signed_phi_a_larger_one_neither() {
    let d = dataset(&counted(&[("a", "x", 1), ("a", "y", 6), ("b", "x", 5), ("b", "y", 2)]));
    let r = compute(&d, &params(&[])).unwrap();
    assert!(matches!(r.continuity_corrected, Some(ChiOutcome::Computed(_))));
    let f = r.fisher.expect("Fisher's test");
    assert!(f.less < f.greater, "fewer in the first cell than expected");
    assert!(matches!(r.phi, Stat::Value(v) if v < 0.0), "{:?}", r.phi);
    let d = dataset(&counted(&[
        ("a", "x", 1),
        ("a", "y", 6),
        ("b", "x", 5),
        ("b", "y", 2),
        ("c", "x", 3),
        ("c", "y", 3),
    ]));
    let r = compute(&d, &params(&[])).unwrap();
    assert!(r.continuity_corrected.is_none() && r.fisher.is_none());
    assert!(matches!(r.phi, Stat::Value(v) if v > 0.0));
    assert!(matches!(r.pearson, ChiOutcome::Computed(t) if t.df == 2.0));
}

#[test]
fn the_rows_order_changes_no_bit() {
    let rows = counted(&[("a", "x", 3), ("b", "y", 2), ("a", "y", 1), ("c", "x", 4), ("b", "x", 5), ("c", "y", 2)]);
    let mut reversed = rows.clone();
    reversed.reverse();
    let one = compute(&dataset(&rows), &params(&[])).unwrap();
    let two = compute(&dataset(&reversed), &params(&[])).unwrap();
    assert_eq!(one.canonical_bytes(), two.canonical_bytes());
}

#[test]
fn the_run_stops_at_its_one_check_point_before_the_arithmetic() {
    let d = dataset(&base());
    let mut seen = Vec::new();
    let out = run(&d, &params(&[]), |i| {
        seen.push(i);
        ControlFlow::Break("stop")
    });
    assert_eq!(out, Err(Halt::Stopped { reason: "stop" }));
    assert_eq!(seen, [0]);
    let mut seen = Vec::new();
    let out = run(&d, &params(&[]), |i| {
        seen.push(i);
        ControlFlow::<()>::Continue(())
    });
    assert!(out.is_ok());
    assert_eq!(seen.len(), check_points());
    // A refused spec reaches no check point.
    let mut reached = false;
    let out = run(&d, &params(&[f64::NAN]), |_| {
        reached = true;
        ControlFlow::<()>::Continue(())
    });
    assert!(matches!(out, Err(Halt::Refused(_))) && !reached);
}

#[test]
fn the_result_has_exactly_its_keys() {
    let r = compute(&dataset(&base()), &params(&[])).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&r.canonical_bytes()).unwrap();
    let keys = |v: &serde_json::Value| -> BTreeSet<String> {
        v.as_object().unwrap().keys().cloned().collect()
    };
    let want = |k: &[&str]| -> BTreeSet<String> { k.iter().map(|s| s.to_string()).collect() };
    assert_eq!(
        keys(&v),
        want(&[
            "rowLevels",
            "columnLevels",
            "frequencies",
            "rowTotals",
            "columnTotals",
            "n",
            "pearson",
            "likelihoodRatio",
            "continuityCorrected",
            "fisher",
            "phi",
            "cramersV",
            "contingencyCoefficient",
            "expectedBelowFive",
            "cellCount",
            "smallestExpected",
            "exclusions",
            "notes",
            "limitations",
        ])
    );
    assert_eq!(keys(&v["rowLevels"][0]), want(&["level", "label"]));
    assert_eq!(
        keys(&v["frequencies"][0]),
        want(&[
            "row",
            "column",
            "observed",
            "expected",
            "rowProportion",
            "columnProportion",
            "totalProportion",
            "residual",
            "pearsonResidual",
            "adjustedResidual",
        ])
    );
    assert_eq!(keys(&v["pearson"]["computed"]), want(&["statistic", "df", "p"]));
    assert_eq!(keys(&v["fisher"]), want(&["twoSided", "less", "greater"]));
    assert_eq!(
        r.limitations,
        [
            XtabCode::NoLayers,
            XtabCode::FisherTwoByTwoOnly,
            XtabCode::NoFurtherNominalMeasures,
            XtabCode::NoWeights,
            XtabCode::CodePointOrder,
        ]
    );
    // The level labels print as counts.
    let debug = format!("{:?}", r.row_levels);
    assert!(debug.contains("label_chars") && !debug.contains("\"a\""), "{debug}");
}

#[test]
fn validate_warns_of_suspect_codes_in_either_column_and_refuses_other_specs() {
    let d = dataset(&counted(&[
        ("1", "1", 3),
        ("2", "2", 3),
        ("1", "2", 2),
        ("2", "1", 2),
        ("99", "1", 2),
        ("1", "-9", 2),
    ]));
    let s = spec(OperationId(3), &d, params(&[]));
    let report = validate(&d, &s);
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let warned: Vec<(XtabCode, Option<i64>, Option<i64>)> = report
        .warnings
        .iter()
        .map(|w| (w.code, int(w, ParamKey::Column), int(w, ParamKey::Value)))
        .collect();
    assert!(
        warned.contains(&(XtabCode::SuspectMissingCode, Some(1), Some(99))),
        "{warned:?}"
    );
    assert!(
        warned.contains(&(XtabCode::SuspectMissingCode, Some(2), Some(-9))),
        "{warned:?}"
    );
    // Listed, a code is not warned of.
    let report = validate(&d, &spec(OperationId(3), &d, params(&[-9.0, 99.0])));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    // Another capability, another version, another workspace.
    let mut other = spec(OperationId(4), &d, params(&[]));
    other.capability_id = CapabilityId::parse("stats.analyze.one_way_anova").unwrap();
    other.capability_version = 2;
    other.workspace_id = WorkspaceId::Compute;
    let report = validate(&d, &other);
    assert_eq!(report.status, ValidationStatus::Unsupported);
    let codes: Vec<XtabCode> = report.errors[..2].iter().map(|u| u.code).collect();
    assert_eq!(
        codes,
        [XtabCode::UnknownCapability, XtabCode::CapabilityVersionMismatch]
    );
    // Another dataset's input; no input.
    let elsewhere = dataset(&base());
    let report = validate(&d, &spec(OperationId(5), &elsewhere, params(&[])));
    assert_eq!(report.errors[0].code, XtabCode::InputMismatch);
    let mut none = spec(OperationId(6), &d, params(&[]));
    none.inputs.clear();
    let report = validate(&d, &none);
    assert_eq!(
        (report.errors[0].code, int(&report.errors[0], ParamKey::Count)),
        (XtabCode::InputCount, Some(0))
    );
}

#[test]
fn the_spec_is_camel_case_and_refuses_unknown_fields() {
    let p: CrosstabsParams =
        serde_json::from_str(r#"{"rows":"C1","columns":"C2","missingCodes":[-9,99]}"#).unwrap();
    assert_eq!(p, params(&[-9.0, 99.0]));
    assert!(serde_json::from_str::<CrosstabsParams>(
        r#"{"rows":"C1","columns":"C2","missingCodes":[],"layer":"C3"}"#
    )
    .is_err());
    assert!(
        serde_json::from_str::<CrosstabsParams>(r#"{"rows":"C1","columns":"C2"}"#).is_err(),
        "missingCodes is required"
    );
}
