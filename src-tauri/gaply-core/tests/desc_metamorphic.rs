//! **Metamorphic, determinism, validation and edge-case tests of the
//! descriptive summary (Research OS, Phase 2A),** on datasets built in the
//! test. File formats (csv, tsv, xlsx, a BOM, CRLF) need the app crate's
//! parser and are Phase 2B's.
//!
//! * Appending an all-missing row changes only the missing counts.
//! * Reordering the columns reorders only the output.
//! * Ten runs give byte-identical result, CSV and manifest bytes.
//! * Every validation finding is the closed code it should be, and
//!   `describe` refuses with the same code.
//! * Every edge case is a closed outcome, and no output holds NaN, an
//!   infinity or a null.

use serde_json::Value;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{
    Arch, ContentHash, OperationId, Os, ParamKey, ParamValue, Platform, Reproducibility,
    ValidationStatus,
};
use gaply_core::stats_desc::export::{manifest_json, result_csv};
use gaply_core::stats_desc::{
    describe, input_ref, manifest, profile, spec, validate, Alias, ColumnKind, ColumnRequest,
    ColumnResult, DescCode, DescribeParams, Stat, TreatAs, CAPABILITY_ID,
};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

fn table(headers: &[&str], rows: &[&[&str]]) -> Dataset {
    let raw = RawTable::from_rows(
        headers.iter().map(|h| h.to_string()).collect(),
        rows.iter()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect(),
    );
    let bytes: Vec<u8> = format!("{headers:?}{rows:?}").into_bytes();
    build_dataset(
        raw,
        DataFingerprint::of_bytes(&bytes, "test", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn request(column: u32, treat_as: TreatAs) -> ColumnRequest {
    ColumnRequest {
        alias: Alias(ColumnAlias(column)),
        treat_as,
    }
}

fn params(columns: &[(u32, TreatAs)], missing_codes: &[f64]) -> DescribeParams {
    DescribeParams {
        columns: columns.iter().map(|&(c, t)| request(c, t)).collect(),
        missing_codes: missing_codes.to_vec(),
    }
}

const PLATFORM: Platform = Platform {
    os: Os::Macos,
    arch: Arch::Aarch64,
};

fn sample() -> Dataset {
    table(
        &["score", "group", "rating"],
        &[
            &["12.5", "a", "3"],
            &["", "b", "-99"],
            &["7.25", "a", "5"],
            &["9", "", "4"],
            &["30.125", "c", "2"],
            &["11", "b", ""],
            &["-4.5", "a", "6"],
            &["NA", "c", "7"],
        ],
    )
}

fn sample_params() -> DescribeParams {
    params(
        &[
            (1, TreatAs::Numeric),
            (2, TreatAs::Categorical),
            (3, TreatAs::Numeric),
        ],
        &[-99.0],
    )
}

fn json(result: &impl serde::Serialize) -> Value {
    serde_json::to_value(result).expect("json")
}

/// Every `missing` count in a result's JSON, plus one.
fn bump_missing(v: &mut Value) {
    match v {
        Value::Object(map) => {
            for (k, x) in map.iter_mut() {
                if k == "missing" {
                    *x = Value::from(x.as_u64().expect("a count") + 1);
                } else {
                    bump_missing(x);
                }
            }
        }
        Value::Array(xs) => xs.iter_mut().for_each(bump_missing),
        _ => {}
    }
}

fn strip_alias(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.remove("alias");
            map.values_mut().for_each(strip_alias);
        }
        Value::Array(xs) => xs.iter_mut().for_each(strip_alias),
        _ => {}
    }
}

/// No NaN, no infinity (serde_json writes both as null), no null at all.
fn no_null(v: &Value) -> bool {
    no_null_but(v, None)
}

/// The same, except under `key`, where null is a state of its own (a
/// profile's `levels` is absent for a numeric column, not empty).
fn no_null_but(v: &Value, key: Option<&str>) -> bool {
    match v {
        Value::Null => false,
        Value::Object(m) => m
            .iter()
            .all(|(k, x)| (Some(k.as_str()) == key && x.is_null()) || no_null_but(x, key)),
        Value::Array(xs) => xs.iter().all(|x| no_null_but(x, key)),
        Value::Number(n) => n.as_f64().is_some_and(f64::is_finite),
        _ => true,
    }
}

#[test]
fn appending_an_all_missing_row_changes_only_the_missing_counts() {
    let base = sample();
    let mut rows: Vec<Vec<&str>> = vec![
        vec!["12.5", "a", "3"],
        vec!["", "b", "-99"],
        vec!["7.25", "a", "5"],
        vec!["9", "", "4"],
        vec!["30.125", "c", "2"],
        vec!["11", "b", ""],
        vec!["-4.5", "a", "6"],
        vec!["NA", "c", "7"],
    ];
    rows.push(vec!["", "", ""]);
    let slices: Vec<&[&str]> = rows.iter().map(Vec::as_slice).collect();
    let longer = table(&["score", "group", "rating"], &slices);

    let p = sample_params();
    let mut before = json(&describe(&base, &p).expect("describes"));
    let after = json(&describe(&longer, &p).expect("describes"));
    bump_missing(&mut before);
    assert_eq!(before, after);

    let (pb, pa) = (profile(&base), profile(&longer));
    for (b, a) in pb.columns.iter().zip(&pa.columns) {
        assert_eq!(
            (b.n, b.missing + 1, b.distinct, &b.levels, b.kind),
            (a.n, a.missing, a.distinct, &a.levels, a.kind)
        );
    }
}

#[test]
fn reordering_the_columns_reorders_only_the_output() {
    let base = sample();
    // Columns in the order rating, score, group.
    let permuted = table(
        &["rating", "score", "group"],
        &[
            &["3", "12.5", "a"],
            &["-99", "", "b"],
            &["5", "7.25", "a"],
            &["4", "9", ""],
            &["2", "30.125", "c"],
            &["", "11", "b"],
            &["6", "-4.5", "a"],
            &["7", "NA", "c"],
        ],
    );
    let all = |d: &Dataset| DescribeParams {
        missing_codes: vec![-99.0],
        ..DescribeParams::defaults_for(&profile(d))
    };
    let mut a = json(&describe(&base, &all(&base)).expect("describes"));
    let mut b = json(&describe(&permuted, &all(&permuted)).expect("describes"));
    strip_alias(&mut a);
    strip_alias(&mut b);
    let a = a["columns"].as_array().expect("columns").clone();
    let b = b["columns"].as_array().expect("columns").clone();
    assert_eq!(vec![a[2].clone(), a[0].clone(), a[1].clone()], b);
}

#[test]
fn ten_runs_give_identical_result_csv_and_manifest_bytes() {
    let d = sample();
    let s = spec(OperationId(5), &d, sample_params());
    let once = || {
        let r = describe(&d, &s.parameters).expect("describes");
        let m = manifest(&s, &d, &r, 17, PLATFORM);
        (r.canonical_bytes(), result_csv(&r), manifest_json(&m))
    };
    let first = once();
    for _ in 0..9 {
        assert_eq!(once(), first);
    }
    // elapsed_ms is the only field a caller varies; set aside, the bytes match.
    let r = describe(&d, &s.parameters).expect("describes");
    let mut m = manifest(&s, &d, &r, 99_999, PLATFORM);
    assert_ne!(manifest_json(&m), first.2);
    m.elapsed_ms = 17;
    assert_eq!(manifest_json(&m), first.2);
}

#[test]
fn the_manifest_records_hashes_versions_counts_and_exclusions() {
    let d = sample();
    let s = spec(OperationId(5), &d, sample_params());
    let r = describe(&d, &s.parameters).expect("describes");
    let m = manifest(&s, &d, &r, 17, PLATFORM);
    assert_eq!(m.capability_id.as_str(), CAPABILITY_ID);
    assert_eq!((m.capability_version, m.engine_version), (1, 1));
    assert_eq!(m.inputs, vec![input_ref(&d).content_hash]);
    assert_eq!(m.inputs[0].as_hex(), d.schema.fingerprint.sha256_hex());
    assert_eq!(
        m.parameters_hash,
        ContentHash::of_bytes(&serde_json::to_vec(&s.parameters).expect("json"))
    );
    assert_eq!(m.output_hash, ContentHash::of_bytes(&r.canonical_bytes()));
    assert_eq!(
        (m.rows_read, m.rows_used),
        (8, 8),
        "no row is excluded as a whole"
    );
    assert_eq!(m.seed, None);
    assert_eq!(m.reproducibility, Reproducibility::Bitwise);
    assert_eq!((m.platform, m.elapsed_ms), (PLATFORM, 17));
    let ex: Vec<(&str, u64, i64)> = m
        .exclusions
        .iter()
        .map(|e| {
            let ParamValue::Int(c) = e.params[&ParamKey::Column] else {
                panic!("a column")
            };
            (e.code.wire(), e.count, c)
        })
        .collect();
    // score: one empty and one NA; group: one empty; rating: one empty and one -99.
    assert_eq!(
        ex,
        [
            ("stats.desc.missing_cell", 2, 1),
            ("stats.desc.missing_cell", 1, 2),
            ("stats.desc.missing_cell", 1, 3),
            ("stats.desc.missing_code", 1, 3),
        ]
    );
    // The result agrees with the exclusions.
    let missing: Vec<u64> = r
        .columns
        .iter()
        .map(|c| match c {
            ColumnResult::Numeric(s) => s.missing,
            ColumnResult::Categorical(s) => s.missing,
        })
        .collect();
    assert_eq!(missing, [2, 1, 2]);
}

#[test]
fn each_validation_finding_is_its_closed_code_and_describe_refuses_with_it() {
    let d = sample();
    let ok = validate(&d, &spec(OperationId(1), &d, sample_params()));
    assert_eq!(ok.status, ValidationStatus::Valid, "{:?}", ok.errors);
    assert_eq!(
        (
            ok.estimated_resources.rows,
            ok.estimated_resources.columns,
            ok.estimated_resources.cells
        ),
        (8, 3, 24)
    );

    let cases: Vec<(DescribeParams, DescCode, ParamKey, i64)> = vec![
        (
            params(&[(9, TreatAs::Numeric)], &[]),
            DescCode::UnknownColumn,
            ParamKey::Column,
            9,
        ),
        (
            params(&[(1, TreatAs::Numeric), (1, TreatAs::Categorical)], &[]),
            DescCode::DuplicateColumn,
            ParamKey::Column,
            1,
        ),
        (
            params(&[(2, TreatAs::Numeric)], &[]),
            DescCode::NumericOnText,
            ParamKey::Count,
            7,
        ),
        (
            params(&[(1, TreatAs::Numeric)], &[-99.0, f64::NAN]),
            DescCode::MissingCodeNotFinite,
            ParamKey::Index,
            1,
        ),
        (
            params(&[(1, TreatAs::Numeric)], &[f64::INFINITY]),
            DescCode::MissingCodeNotFinite,
            ParamKey::Index,
            0,
        ),
    ];
    for (p, code, key, value) in cases {
        let report = validate(&d, &spec(OperationId(1), &d, p.clone()));
        assert_eq!(report.status, ValidationStatus::Invalid, "{code:?}");
        assert_eq!(report.errors[0].code, code);
        assert_eq!(
            report.errors[0].params.get(&key),
            Some(&ParamValue::Int(value)),
            "{code:?}"
        );
        let refused = describe(&d, &p).expect_err("refused");
        assert_eq!(refused.code, code);
    }

    let none = validate(&d, &spec(OperationId(1), &d, params(&[], &[])));
    assert_eq!(none.status, ValidationStatus::NeedsUserInput);
    assert_eq!(
        none.missing_information[0].code,
        DescCode::NoColumnsSelected
    );
    assert_eq!(
        describe(&d, &params(&[], &[])).expect_err("refused").code,
        DescCode::NoColumnsSelected
    );

    let mut other = spec(OperationId(1), &d, sample_params());
    other.capability_id =
        gaply_core::contracts::CapabilityId::parse("stats.frequencies").expect("id");
    assert_eq!(validate(&d, &other).status, ValidationStatus::Unsupported);
    let mut later = spec(OperationId(1), &d, sample_params());
    later.capability_version = 2;
    let r = validate(&d, &later);
    assert_eq!(
        (r.status, r.errors[0].code),
        (
            ValidationStatus::Unsupported,
            DescCode::CapabilityVersionMismatch
        )
    );
    assert_eq!(
        r.errors[0].params.get(&ParamKey::Found),
        Some(&ParamValue::Int(2))
    );

    let elsewhere = table(&["x"], &[&["1"], &["2"]]);
    let mut wrong_input = spec(OperationId(1), &d, sample_params());
    wrong_input.inputs = vec![input_ref(&elsewhere)];
    assert_eq!(
        validate(&d, &wrong_input).errors[0].code,
        DescCode::InputMismatch
    );
    wrong_input.inputs.clear();
    let r = validate(&d, &wrong_input);
    assert_eq!(
        (r.errors[0].code, r.errors[0].params.get(&ParamKey::Count)),
        (DescCode::InputCount, Some(&ParamValue::Int(0)))
    );

    // A suspect code the user did not list warns; it is never applied.
    let likert = table(
        &["id", "x"],
        &[
            &["1", "1"],
            &["2", "2"],
            &["3", "3"],
            &["4", "-99"],
            &["5", "4"],
            &["6", "5"],
        ],
    );
    let w = validate(
        &likert,
        &spec(
            OperationId(1),
            &likert,
            params(&[(2, TreatAs::Numeric)], &[]),
        ),
    );
    assert_eq!(w.status, ValidationStatus::ValidWithWarnings);
    assert_eq!(
        w.warnings[0].params.get(&ParamKey::Value),
        Some(&ParamValue::Int(-99))
    );
    let unapplied = describe(&likert, &params(&[(2, TreatAs::Numeric)], &[])).expect("describes");
    let ColumnResult::Numeric(s) = &unapplied.columns[0] else {
        panic!("numeric")
    };
    assert_eq!(
        (s.n, s.missing, s.min),
        (6, 0, Stat::Value(-99.0)),
        "the profile's candidate is not applied"
    );
}

#[test]
fn every_edge_case_is_a_closed_outcome_with_no_nan_infinity_or_null() {
    let d = table(
        &[
            "empty", "single", "constant", "huge", "tiny", "infinite", "nan",
        ],
        &[
            &["", "4.5", "0.1", "1e308", "0", "1", "1"],
            &["", "", "0.1", "1.5e308", "1e-200", "inf", "NaN"],
            &["", "", "0.1", "1e308", "0", "2", "3"],
        ],
    );
    let p = profile(&d);
    assert_eq!(p.columns[0].kind, ColumnKind::Empty);
    assert_eq!(
        p.columns[5].kind,
        ColumnKind::Categorical,
        "a non-finite cell types the column as text"
    );
    assert_eq!(p.columns[5].non_finite, 1);
    assert_eq!(
        (p.columns[6].kind, p.columns[6].n, p.columns[6].missing),
        (ColumnKind::Numeric, 2, 1),
        "NaN is a missing-value token"
    );

    let numeric = |c: u32| {
        let r = describe(&d, &params(&[(c, TreatAs::Numeric)], &[])).expect("describes");
        assert!(no_null(&json(&r)), "column {c}");
        match r.columns.into_iter().next().expect("one") {
            ColumnResult::Numeric(s) => s,
            other => panic!("{other:?}"),
        }
    };
    let none = Stat::NotComputable(DescCode::NoValues);
    let e = numeric(1);
    assert_eq!((e.n, e.missing), (0, 3));
    assert!([e.mean, e.sd, e.min, e.q1, e.median, e.q3, e.max]
        .iter()
        .all(|s| *s == none));

    let one = numeric(2);
    assert_eq!(one.sd, Stat::NotComputable(DescCode::SingleValue));
    assert!([one.mean, one.min, one.q1, one.median, one.q3, one.max]
        .iter()
        .all(|s| *s == Stat::Value(4.5)));

    let c = numeric(3);
    let Stat::Value(sd) = c.sd else { panic!("sd") };
    assert_eq!(
        sd.to_bits(),
        0f64.to_bits(),
        "a column whose min equals its max has sd exactly 0"
    );

    let h = numeric(4);
    assert_eq!(
        (h.mean, h.sd),
        (
            Stat::NotComputable(DescCode::Overflow),
            Stat::NotComputable(DescCode::Overflow)
        )
    );
    assert_eq!((h.min, h.max), (Stat::Value(1e308), Stat::Value(1.5e308)));

    let t = numeric(5);
    assert_eq!(
        t.sd,
        Stat::NotComputable(DescCode::Underflow),
        "squares below the smallest normal double"
    );
    assert!(matches!(t.mean, Stat::Value(m) if m > 0.0));

    let refused =
        describe(&d, &params(&[(6, TreatAs::Numeric)], &[])).expect_err("a non-finite cell");
    assert_eq!(refused.code, DescCode::NonFiniteCell);
    assert_eq!(
        refused.params.get(&ParamKey::Count),
        Some(&ParamValue::Int(1))
    );
    let as_levels =
        describe(&d, &params(&[(6, TreatAs::Categorical)], &[])).expect("as categorical");
    let ColumnResult::Categorical(s) = &as_levels.columns[0] else {
        panic!("categorical")
    };
    assert_eq!(
        s.levels
            .iter()
            .map(|l| (l.label.as_str(), l.count))
            .collect::<Vec<_>>(),
        [("1", 1), ("2", 1), ("inf", 1)]
    );

    let empty_levels = describe(&d, &params(&[(1, TreatAs::Categorical)], &[])).expect("describes");
    let ColumnResult::Categorical(s) = &empty_levels.columns[0] else {
        panic!("categorical")
    };
    assert_eq!((s.n, s.missing, s.levels.len()), (0, 3, 0));
    assert!(no_null_but(&json(&profile(&d)), Some("levels")));
}

#[test]
fn levels_follow_level_1s_codes_in_code_point_order() {
    let d = table(
        &["g"],
        &[
            &["b"],
            &["B"],
            &["1.0"],
            &["1"],
            &["10"],
            &["9"],
            &["é"],
            &["_x"],
            &[" b "],
        ],
    );
    let r = describe(&d, &params(&[(1, TreatAs::Categorical)], &[])).expect("describes");
    let ColumnResult::Categorical(s) = &r.columns[0] else {
        panic!("categorical")
    };
    let got: Vec<(&str, u64)> = s
        .levels
        .iter()
        .map(|l| (l.label.as_str(), l.count))
        .collect();
    // `1` and `1.0` are one level, and text is trimmed, as Level 1 reads them.
    assert_eq!(
        got,
        [
            ("1", 2),
            ("10", 1),
            ("9", 1),
            ("B", 1),
            ("_x", 1),
            ("b", 2),
            ("é", 1)
        ]
    );
}

/// A column whose min equals its max has sd exactly 0, decided before any
/// arithmetic. On ordinary values the two-pass mean already returns the
/// value exactly, so the deviations are exact zeros and the guard is not
/// what makes sd 0 (deleting it leaves every other test green: measured,
/// Phase 2A report). That premise is pinned here, over many values and
/// lengths, so the day the mean changes this goes red. Where the guard IS
/// what decides, a constant column whose sum overflows, sd is still 0.
#[test]
fn a_constant_column_has_sd_exactly_0_before_any_arithmetic() {
    let values = [
        "0.1",
        "0.7",
        "0.3333333333333333",
        "1000000000.1",
        "123456.789",
        "9.95",
        "-2.5",
        "1e-300",
        "1e300",
        "7",
    ];
    let mut checked = 0;
    for v in values {
        for n in 2..=40 {
            let rows: Vec<Vec<&str>> = (0..n).map(|_| vec![v]).collect();
            let slices: Vec<&[&str]> = rows.iter().map(Vec::as_slice).collect();
            let d = table(&["x"], &slices);
            let r = describe(&d, &params(&[(1, TreatAs::Numeric)], &[])).expect("describes");
            let ColumnResult::Numeric(s) = &r.columns[0] else {
                panic!("numeric")
            };
            let value: f64 = v.parse().expect("a number");
            assert_eq!(
                s.mean,
                Stat::Value(value),
                "the premise: the mean of {n} × {v} is {v} exactly"
            );
            let Stat::Value(sd) = s.sd else {
                panic!("{n} × {v}: sd {:?}", s.sd)
            };
            assert_eq!(sd.to_bits(), 0f64.to_bits(), "{n} × {v}");
            checked += 1;
        }
    }
    assert_eq!(checked, 390);

    let d = table(&["x"], &[&["1.5e308"], &["1.5e308"]]);
    let r = describe(&d, &params(&[(1, TreatAs::Numeric)], &[])).expect("describes");
    let ColumnResult::Numeric(s) = &r.columns[0] else {
        panic!("numeric")
    };
    assert_eq!(
        s.mean,
        Stat::NotComputable(DescCode::Overflow),
        "the sum overflows"
    );
    assert_eq!(
        s.sd,
        Stat::Value(0.0),
        "and sd is still exactly 0: decided before the arithmetic"
    );
    assert_eq!(
        (s.min, s.median, s.max),
        (
            Stat::Value(1.5e308),
            Stat::Value(1.5e308),
            Stat::Value(1.5e308)
        )
    );
}

/// Evaluation of Phases 2 and 3, F5 (decision 38): a ticked missing code
/// applies to Numeric requests only. A categorical column that has a "-99"
/// level keeps it, counts and all, and the manifest lists no coded exclusion
/// for it; a numeric column drops its -99 cells and the manifest says how
/// many. The screen's question already says "in the columns you summarise as
/// numeric".
#[test]
fn a_ticked_missing_code_applies_to_numeric_requests_only() {
    // `coded` holds codes as numbers; `label` is text with "-99" among its
    // labels, which the dataset types as a number cell.
    let d = table(
        &["score", "coded", "label"],
        &[
            &["12.5", "1", "x"],
            &["-99", "-99", "-99"],
            &["7.25", "2", "y"],
            &["9", "-99", "x"],
            &["30.125", "1", "y"],
        ],
    );
    let p = params(
        &[
            (1, TreatAs::Numeric),
            (2, TreatAs::Categorical),
            (3, TreatAs::Categorical),
        ],
        &[-99.0],
    );
    let r = describe(&d, &p).expect("describes");

    let ColumnResult::Numeric(score) = &r.columns[0] else {
        panic!("score is numeric")
    };
    assert_eq!((score.n, score.missing), (4, 1), "the -99 cell is left out");
    assert_eq!(score.min, Stat::Value(7.25), "-99 is not the minimum");

    for (i, minus_99) in [(1usize, 2u64), (2, 1)] {
        let ColumnResult::Categorical(c) = &r.columns[i] else {
            panic!("column {i} is categorical")
        };
        assert_eq!((c.n, c.missing), (5, 0), "column {i} keeps every cell");
        let level = c
            .levels
            .iter()
            .find(|l| l.label == "-99")
            .unwrap_or_else(|| panic!("column {i} keeps its -99 level: {:?}", c.levels));
        assert_eq!(level.count, minus_99, "column {i}");
    }

    // The manifest records the numeric column's coded exclusion only.
    let s = spec(OperationId(9), &d, p);
    let m = manifest(&s, &d, &r, 0, PLATFORM);
    let coded: Vec<_> = m
        .exclusions
        .iter()
        .filter(|e| e.code == DescCode::MissingCode)
        .collect();
    assert_eq!(coded.len(), 1, "{:?}", m.exclusions);
    assert_eq!(coded[0].count, 1);
    let numeric = m
        .exclusions
        .iter()
        .find(|e| e.code == DescCode::MissingCell || e.code == DescCode::MissingCode)
        .expect("an exclusion");
    assert_eq!(
        coded[0].params.get(&ParamKey::Column),
        numeric.params.get(&ParamKey::Column),
        "the coded exclusion is the score column's"
    );
}
