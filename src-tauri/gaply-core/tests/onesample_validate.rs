//! **The one-sample and paired t-tests' validation, data rules, check point,
//! determinism and exports (Research OS, Phase 8D-1, decisions 255 and 256).**
//!
//! * The test value is never defaulted: until it is given, validation needs
//!   input (`test_value_not_chosen`), and a run is refused with that code. A
//!   test value that is not finite is invalid.
//! * The same column twice is invalid; text and non-finite cells in a chosen
//!   column are invalid, with the column; a missing code that is not finite is
//!   invalid; unsupported capability, version and input are worded in this
//!   module's codes.
//! * Each row is left out once, by the first reason that applies, in decision
//!   255's order.
//! * df 10,000 and 10,001 both compute (Research OS, Phase 8E: above 1e4 the
//!   p and the quantile are `stats_dist`'s); fewer than two values or pairs is
//!   refused at validation.
//! * The check point is called once, before the arithmetic, and a `Break`
//!   stops the run there.
//! * Ten runs give byte-identical result and manifest bytes, `elapsed_ms` set
//!   aside. The CSV reads back field by field; the manifest JSON holds no
//!   header, cell or path (sentinels).

use std::ops::ControlFlow;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{
    Arch, OperationId, Os, ParamKey, ParamValue, Platform, ValidationStatus,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_onesample::{
    export, one_sample_compute, one_sample_manifest, one_sample_run, one_sample_spec,
    one_sample_validate, paired_compute, paired_manifest, paired_run, paired_spec, paired_validate,
    Halt, OneSampleCode, OneSampleParams, PairedParams,
};

/// Headers that must never reach a manifest or a refusal.
const HEADERS: [&str; 3] = [
    "HTSENTINEL_head_a",
    "HTSENTINEL_head_b",
    "HTSENTINEL_head_c",
];

fn dataset(rows: &[[&str; 3]]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            HEADERS.iter().map(|h| h.to_string()).collect(),
            rows.iter()
                .map(|r| r.iter().map(|c| c.to_string()).collect())
                .collect(),
        ),
        DataFingerprint::of_bytes(b"onesample validate", "test", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn a(i: u32) -> Alias {
    Alias(ColumnAlias(i))
}

const ROWS: &[[&str; 3]] = &[
    ["4.5", "3.25", "x"],
    ["6", "5.5", "y"],
    ["5.25", "4", "x"],
    ["7.75", "6.5", "y"],
    ["3.5", "3.75", "x"],
    ["6.25", "4.5", "y"],
];

fn one_params(test_value: Option<f64>, codes: Vec<f64>) -> OneSampleParams {
    OneSampleParams {
        outcome: a(1),
        test_value,
        missing_codes: codes,
    }
}

fn paired_params(first: u32, second: u32, codes: Vec<f64>) -> PairedParams {
    PairedParams {
        first: a(first),
        second: a(second),
        missing_codes: codes,
    }
}

#[test]
fn the_test_value_is_never_defaulted_and_must_be_finite() {
    let d = dataset(ROWS);
    let p = one_params(None, vec![]);
    let v = one_sample_validate(&d, &one_sample_spec(OperationId(1), &d, p.clone()));
    assert_eq!(v.status, ValidationStatus::NeedsUserInput);
    assert_eq!(
        v.missing_information[0].code,
        OneSampleCode::TestValueNotChosen
    );
    assert_eq!(
        one_sample_compute(&d, &p).unwrap_err().code,
        OneSampleCode::TestValueNotChosen,
        "a run without a test value is refused, not run at 0"
    );
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let v = one_sample_validate(
            &d,
            &one_sample_spec(OperationId(2), &d, one_params(Some(bad), vec![])),
        );
        assert_eq!(v.status, ValidationStatus::Invalid);
        assert_eq!(v.errors[0].code, OneSampleCode::TestValueNotFinite);
    }
    let v = one_sample_validate(
        &d,
        &one_sample_spec(OperationId(3), &d, one_params(Some(5.0), vec![])),
    );
    assert_eq!(v.status, ValidationStatus::Valid, "{:?}", v.errors);
}

#[test]
fn parameter_errors_are_invalid_with_their_codes() {
    let d = dataset(ROWS);
    let codes = |r: &gaply_core::contracts::ValidationReport<OneSampleCode>| {
        r.errors.iter().map(|e| e.code).collect::<Vec<_>>()
    };
    // The same column twice.
    let v = paired_validate(
        &d,
        &paired_spec(OperationId(1), &d, paired_params(1, 1, vec![])),
    );
    assert_eq!(codes(&v), [OneSampleCode::SameColumn]);
    // A column of text, in either test, named by its alias.
    let v = paired_validate(
        &d,
        &paired_spec(OperationId(2), &d, paired_params(1, 3, vec![])),
    );
    assert_eq!(codes(&v), [OneSampleCode::ColumnHoldsText]);
    assert_eq!(
        v.errors[0].params.get(&ParamKey::Column),
        Some(&ParamValue::Int(3))
    );
    let mut text = one_params(Some(1.0), vec![]);
    text.outcome = a(3);
    let v = one_sample_validate(&d, &one_sample_spec(OperationId(3), &d, text));
    assert_eq!(codes(&v), [OneSampleCode::ColumnHoldsText]);
    // An unknown column.
    let v = paired_validate(
        &d,
        &paired_spec(OperationId(4), &d, paired_params(1, 9, vec![])),
    );
    assert_eq!(codes(&v), [OneSampleCode::UnknownColumn]);
    // A missing code that is not finite.
    let v = one_sample_validate(
        &d,
        &one_sample_spec(OperationId(5), &d, one_params(Some(1.0), vec![f64::NAN])),
    );
    assert_eq!(codes(&v), [OneSampleCode::MissingCodeNotFinite]);
    // Non-finite cells.
    let inf = dataset(&[["inf", "1", "x"], ["2", "3", "y"], ["4", "5", "x"]]);
    let v = one_sample_validate(
        &inf,
        &one_sample_spec(OperationId(6), &inf, one_params(Some(1.0), vec![])),
    );
    assert_eq!(codes(&v), [OneSampleCode::ColumnNonFinite]);
    // Another capability, a wrong version, another input.
    let mut s = one_sample_spec(OperationId(7), &d, one_params(Some(1.0), vec![]));
    s.capability_version = 2;
    let v = one_sample_validate(&d, &s);
    assert_eq!(v.status, ValidationStatus::Unsupported);
    // Unsupported findings come first among the errors (ValidationReport::new).
    assert_eq!(v.errors[0].code, OneSampleCode::CapabilityVersionMismatch);
    let s = paired_spec(
        OperationId(8),
        &dataset(&ROWS[..4]),
        paired_params(1, 2, vec![]),
    );
    assert_eq!(
        codes(&paired_validate(&d, &s)),
        [OneSampleCode::InputMismatch]
    );
    let mut s = paired_spec(OperationId(9), &d, paired_params(1, 2, vec![]));
    s.capability_id = gaply_core::stats_onesample::one_sample_capability_id();
    let v = paired_validate(&d, &s);
    assert_eq!(v.status, ValidationStatus::Unsupported);
    assert_eq!(v.errors[0].code, OneSampleCode::UnknownCapability);
}

#[test]
fn each_row_is_left_out_once_by_the_first_reason_in_order() {
    let d = dataset(&[
        ["", "1", "x"],      // missing_first (and missing_value for x)
        ["2", "", "x"],      // missing_second
        ["", "", "x"],       // missing_first, once
        ["-99", "3", "x"],   // missing_code
        ["4", "-99", "x"],   // missing_code
        ["-99", "-99", "x"], // missing_code, once
        ["5", "6", "x"],
        ["7", "6.5", "x"],
        ["8.5", "7", "x"],
    ]);
    let r = paired_compute(&d, &paired_params(1, 2, vec![-99.0])).expect("computes");
    let count = |c| {
        r.exclusions
            .iter()
            .find(|e| e.code == c)
            .map_or(0, |e| e.count)
    };
    assert_eq!(
        (
            count(OneSampleCode::MissingFirst),
            count(OneSampleCode::MissingSecond),
            count(OneSampleCode::MissingCode)
        ),
        (2, 1, 3)
    );
    assert_eq!(r.n_pairs, 3);
    let o = one_sample_compute(&d, &one_params(Some(0.0), vec![-99.0])).expect("computes");
    let count = |c| {
        o.exclusions
            .iter()
            .find(|e| e.code == c)
            .map_or(0, |e| e.count)
    };
    assert_eq!(
        (
            count(OneSampleCode::MissingValue),
            count(OneSampleCode::MissingCode)
        ),
        (2, 2)
    );
    assert_eq!(o.n, 5);
    // The rows read add up.
    let m = paired_manifest(
        &paired_spec(OperationId(1), &d, paired_params(1, 2, vec![-99.0])),
        &d,
        &r,
        0,
        Platform::current(),
    );
    assert_eq!(
        m.rows_read,
        m.rows_used + m.exclusions.iter().map(|e| e.count).sum::<u64>()
    );
}

fn column_of(n: usize, f: impl Fn(usize) -> f64) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            vec!["x".into(), "y".into(), "z".into()],
            (0..n)
                .map(|i| {
                    vec![
                        format!("{}", f(i)),
                        format!("{}", f(i) * 0.5 + 1.0),
                        "t".into(),
                    ]
                })
                .collect(),
        ),
        DataFingerprint::of_bytes(b"df boundary", "test", TableRef::FirstTable),
    )
    .expect("dataset")
}

#[test]
fn df_10000_and_10001_both_compute_and_too_few_is_refused_at_validation() {
    let v = |i: usize| ((i * 7919) % 1000) as f64 / 8.0;
    // 10,001 values give df 10,000, the bisection's last df; 10,002 give
    // 10,001, the first the large-sample formulas serve (Research OS, Phase
    // 8E, decision 275). Both compute; before 8E the second was refused.
    for n in [10_001usize, 10_002] {
        let d = column_of(n, v);
        let df = (n - 1) as f64;
        let one = one_sample_compute(&d, &one_params(Some(60.0), vec![]));
        let two = paired_compute(&d, &paired_params(1, 2, vec![]));
        assert_eq!(one.expect("computes").df, df);
        assert_eq!(two.expect("computes").df, df);
    }
    let d = column_of(1, v);
    let r = one_sample_validate(
        &d,
        &one_sample_spec(OperationId(1), &d, one_params(Some(1.0), vec![])),
    );
    assert_eq!(r.errors[0].code, OneSampleCode::TooFewValues);
    let r = paired_validate(
        &d,
        &paired_spec(OperationId(2), &d, paired_params(1, 2, vec![])),
    );
    assert_eq!(r.errors[0].code, OneSampleCode::TooFewPairs);
}

#[test]
fn the_check_point_is_called_once_before_the_arithmetic_and_a_break_stops_there() {
    let d = dataset(ROWS);
    let mut calls = Vec::new();
    let r = one_sample_run(&d, &one_params(Some(5.0), vec![]), |k| {
        calls.push(k);
        ControlFlow::<()>::Continue(())
    });
    assert!(r.is_ok());
    assert_eq!(calls, [0]);
    assert_eq!(
        one_sample_run(&d, &one_params(Some(5.0), vec![]), |_| ControlFlow::Break(
            "stop"
        ))
        .unwrap_err(),
        Halt::Stopped { reason: "stop" }
    );
    assert_eq!(
        paired_run(&d, &paired_params(1, 2, vec![]), |_| ControlFlow::Break(7)).unwrap_err(),
        Halt::Stopped { reason: 7 }
    );
    // A refused spec never reaches the check point.
    let mut reached = false;
    let _ = paired_run(&d, &paired_params(1, 1, vec![]), |_| {
        reached = true;
        ControlFlow::<()>::Continue(())
    });
    assert!(!reached);
}

#[test]
fn ten_runs_give_identical_result_and_manifest_bytes() {
    let d = dataset(ROWS);
    let platform = Platform {
        os: Os::Macos,
        arch: Arch::Aarch64,
    };
    let one_spec = one_sample_spec(OperationId(1), &d, one_params(Some(5.0), vec![]));
    let two_spec = paired_spec(OperationId(2), &d, paired_params(1, 2, vec![]));
    let mut seen: Option<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> = None;
    for i in 0..10u64 {
        let o = one_sample_compute(&d, &one_spec.parameters).unwrap();
        let p = paired_compute(&d, &two_spec.parameters).unwrap();
        // elapsed_ms set aside: each run passes its own, then it is zeroed.
        let mut mo = one_sample_manifest(&one_spec, &d, &o, i, platform);
        let mut mp = paired_manifest(&two_spec, &d, &p, i * 3, platform);
        mo.elapsed_ms = 0;
        mp.elapsed_ms = 0;
        let now = (
            o.canonical_bytes(),
            p.canonical_bytes(),
            export::manifest_json(&mo),
            export::manifest_json(&mp),
        );
        match &seen {
            None => seen = Some(now),
            Some(first) => assert!(*first == now, "run {i} differs"),
        }
    }
}

/// One CSV line's fields (the writer quotes text fields and leaves numbers
/// bare; no field here holds a comma).
fn fields(line: &str) -> Vec<String> {
    line.split(',')
        .map(|f| f.trim_matches('"').to_string())
        .collect()
}

#[test]
fn the_csv_reads_back_field_by_field_and_the_manifest_holds_no_header_cell_or_path() {
    let d = dataset(ROWS);
    let o = one_sample_compute(&d, &one_params(Some(5.0), vec![])).unwrap();
    let csv = String::from_utf8(export::one_sample_result_csv(&o)).unwrap();
    let lines: Vec<&str> = csv.split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(lines[0], "\"statistic\",\"column\",\"value\",\"word\"");
    let row = |name: &str| -> Vec<String> {
        lines
            .iter()
            .map(|l| fields(l))
            .find(|f| f[0] == name)
            .unwrap_or_else(|| panic!("{name}"))
    };
    assert_eq!(row("n")[2], o.n.to_string());
    for (name, v) in [
        ("mean", o.mean),
        ("sd", o.sd),
        ("test_value", o.test_value),
        ("t", o.t),
        ("df", o.df),
        ("p", o.p),
        ("mean_difference", o.mean_difference),
        ("standard_error", o.standard_error),
        ("confidence_level", o.confidence_level),
        ("ci_lower", o.ci_lower),
        ("ci_upper", o.ci_upper),
        ("cohens_d", o.cohens_d),
    ] {
        assert_eq!(
            row(name)[2].parse::<f64>().unwrap().to_bits(),
            v.to_bits(),
            "{name}"
        );
        assert_eq!(row(name)[1], "", "{name}: no column");
    }
    let limitations: Vec<String> = lines
        .iter()
        .map(|l| fields(l))
        .filter(|f| f[0] == "limitation")
        .map(|f| f[3].clone())
        .collect();
    assert_eq!(
        limitations,
        o.limitations
            .iter()
            .map(|c| c.wire().to_string())
            .collect::<Vec<_>>()
    );

    let p = paired_compute(&d, &paired_params(1, 2, vec![])).unwrap();
    let csv = String::from_utf8(export::paired_result_csv(&p)).unwrap();
    let rows: Vec<Vec<String>> = csv
        .split("\r\n")
        .filter(|l| !l.is_empty())
        .skip(1)
        .map(fields)
        .collect();
    let find = |name: &str, column: &str| -> f64 {
        rows.iter()
            .find(|f| f[0] == name && f[1] == column)
            .unwrap_or_else(|| panic!("{name} {column}"))[2]
            .parse()
            .unwrap()
    };
    assert_eq!(find("mean", "first").to_bits(), p.first.mean.to_bits());
    assert_eq!(find("sd", "second").to_bits(), p.second.sd.to_bits());
    assert_eq!(find("cohens_dz", "").to_bits(), p.cohens_dz.to_bits());
    assert_eq!(
        find("sd_differences", "").to_bits(),
        p.sd_differences.to_bits()
    );

    // The manifest and the results' bytes name no header and no cell text.
    let spec = paired_spec(OperationId(1), &d, paired_params(1, 2, vec![]));
    let m = export::manifest_json(&paired_manifest(&spec, &d, &p, 5, Platform::current()));
    let text = String::from_utf8(m).unwrap();
    for bytes in [
        text.as_bytes().to_vec(),
        p.canonical_bytes(),
        o.canonical_bytes(),
    ] {
        let s = String::from_utf8(bytes).unwrap();
        assert!(!s.contains("HTSENTINEL"), "a header reached the bytes: {s}");
        assert!(
            !s.contains('/'),
            "a path-like string reached the bytes: {s}"
        );
    }
}
