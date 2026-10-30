//! **What the t-test refuses, asks and leaves out (Research OS, Phase 4B-1).**
//!
//! Decisions 64 and 65, one test each: the variant is never chosen for the
//! user (NeedsUserInput); a group of more than two levels needs the two
//! chosen; each Invalid code; rows left out by reason and counted once; a
//! group too small after the exclusions; both groups constant; one constant
//! group computed under both variants; the sign is group 1 minus group 2 in
//! code-point order; the check point runs once, before the arithmetic.

use std::ops::ControlFlow;

use gaply_core::contracts::{CapabilityId, OperationId, ParamKey, ParamValue, ValidationStatus};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_ttest::{
    compute, group_levels, run, spec, t_quantile_975, validate, Halt, Stat, TTestCode, TTestParams,
    Variant,
};

fn table(headers: &[&str], rows: &[&[&str]]) -> Dataset {
    let raw = RawTable::from_rows(
        headers.iter().map(|h| h.to_string()).collect(),
        rows.iter()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect(),
    );
    build_dataset(
        raw,
        DataFingerprint::of_bytes(
            format!("{headers:?}{rows:?}").as_bytes(),
            "test",
            TableRef::FirstTable,
        ),
    )
    .expect("dataset")
}

fn p(levels: Option<[u32; 2]>, variant: Option<Variant>) -> TTestParams {
    TTestParams {
        outcome: Alias(ColumnAlias(1)),
        group: Alias(ColumnAlias(2)),
        levels,
        variant,
        missing_codes: vec![],
    }
}

fn two_levels() -> Dataset {
    table(
        &["y", "g"],
        &[
            &["1", "b"],
            &["2", "b"],
            &["4", "b"],
            &["5", "a"],
            &["7", "a"],
            &["8.5", "a"],
        ],
    )
}

fn three_levels() -> Dataset {
    table(
        &["y", "g"],
        &[
            &["1", "x"],
            &["2", "x"],
            &["3", "y"],
            &["5", "y"],
            &["8", "z"],
            &["13", "z"],
            &["21", "z"],
        ],
    )
}

fn status(d: &Dataset, params: TTestParams) -> (ValidationStatus, Vec<TTestCode>, Vec<TTestCode>) {
    let r = validate(d, &spec(OperationId(1), d, params));
    (
        r.status,
        r.errors.iter().map(|e| e.code).collect(),
        r.missing_information.iter().map(|e| e.code).collect(),
    )
}

#[test]
fn the_variant_is_never_chosen_for_the_user() {
    let d = two_levels();
    let (s, errors, asks) = status(&d, p(None, None));
    assert_eq!(s, ValidationStatus::NeedsUserInput);
    assert!(errors.is_empty());
    assert_eq!(asks, [TTestCode::VariantNotChosen]);
    assert_eq!(
        compute(&d, &p(None, None)).unwrap_err().code,
        TTestCode::VariantNotChosen,
        "a run with the question open is refused, never defaulted"
    );
    for v in [Variant::Student, Variant::Welch] {
        let (s, _, _) = status(&d, p(None, Some(v)));
        assert_eq!(s, ValidationStatus::Valid, "{v:?}");
        assert_eq!(compute(&d, &p(None, Some(v))).expect("computes").variant, v);
    }
}

#[test]
fn more_than_two_levels_need_the_two_chosen_and_the_rest_are_left_out() {
    let d = three_levels();
    let (s, _, asks) = status(&d, p(None, Some(Variant::Welch)));
    assert_eq!(s, ValidationStatus::NeedsUserInput);
    assert_eq!(asks, [TTestCode::LevelsNotChosen]);
    let r = validate(&d, &spec(OperationId(1), &d, p(None, Some(Variant::Welch))));
    assert_eq!(
        r.missing_information[0].params.get(&ParamKey::Count),
        Some(&ParamValue::Int(3))
    );
    let (s, _, asks) = status(&d, p(None, None));
    assert_eq!(s, ValidationStatus::NeedsUserInput);
    assert_eq!(
        asks,
        [TTestCode::LevelsNotChosen, TTestCode::VariantNotChosen]
    );

    let r = compute(&d, &p(Some([0, 2]), Some(Variant::Student))).expect("computes");
    assert_eq!(
        (r.groups[0].label.as_str(), r.groups[1].label.as_str()),
        ("x", "z")
    );
    assert_eq!((r.groups[0].n, r.groups[1].n), (2, 3));
    assert_eq!(r.exclusions.len(), 1);
    assert_eq!(
        (r.exclusions[0].code, r.exclusions[0].count),
        (TTestCode::OtherLevel, 2)
    );
}

#[test]
fn group_one_is_the_first_level_in_code_point_order_and_the_difference_is_one_minus_two() {
    // "a" sorts before "b" whatever the row order, and "B" before "a".
    let d = two_levels();
    let r = compute(&d, &p(None, Some(Variant::Welch))).expect("computes");
    assert_eq!(r.groups[0].label, "a");
    assert_eq!(r.groups[0].mean, (5.0 + 7.0 + 8.5) / 3.0);
    assert!(r.mean_difference > 0.0 && r.t > 0.0 && r.cohens_d > 0.0);
    assert_eq!(r.mean_difference, r.groups[0].mean - r.groups[1].mean);
    let upper = table(
        &["y", "g"],
        &[&["1", "a"], &["2", "a"], &["5", "B"], &["7", "B"]],
    );
    let r = compute(&upper, &p(None, Some(Variant::Student))).expect("computes");
    assert_eq!((r.groups[0].label.as_str(), r.groups[0].level), ("B", 0));
    assert!(r.mean_difference > 0.0);
}

#[test]
fn each_invalid_code() {
    let d = two_levels();
    let welch = Some(Variant::Welch);
    let cases: Vec<(TTestParams, TTestCode)> = vec![
        (
            TTestParams {
                outcome: Alias(ColumnAlias(9)),
                ..p(None, welch)
            },
            TTestCode::UnknownColumn,
        ),
        (
            TTestParams {
                group: Alias(ColumnAlias(1)),
                ..p(None, welch)
            },
            TTestCode::SameColumn,
        ),
        (p(Some([0, 2]), welch), TTestCode::LevelOutOfRange),
        (p(Some([1, 1]), welch), TTestCode::LevelsEqual),
        (
            TTestParams {
                missing_codes: vec![f64::NAN],
                ..p(None, welch)
            },
            TTestCode::MissingCodeNotFinite,
        ),
    ];
    for (params, code) in cases {
        let (s, errors, _) = status(&d, params.clone());
        assert_eq!(s, ValidationStatus::Invalid, "{code:?}");
        assert_eq!(errors, [code]);
        assert_eq!(compute(&d, &params).unwrap_err().code, code);
    }
    let text = table(
        &["y", "g"],
        &[&["1", "a"], &["n/a?", "a"], &["3", "b"], &["4", "b"]],
    );
    assert_eq!(
        status(&text, p(None, welch)).1,
        [TTestCode::OutcomeHoldsText]
    );
    let one = table(&["y", "g"], &[&["1", "a"], &["2", "a"], &["3", ""]]);
    assert_eq!(status(&one, p(None, welch)).1, [TTestCode::GroupOneLevel]);
    let r = validate(&d, &spec(OperationId(1), &d, p(Some([0, 5]), welch)));
    assert_eq!(
        r.errors[0].params.get(&ParamKey::Index),
        Some(&ParamValue::Int(5))
    );
    assert_eq!(
        r.errors[0].params.get(&ParamKey::Limit),
        Some(&ParamValue::Int(1))
    );
}

#[test]
fn another_capability_or_version_is_unsupported() {
    let d = two_levels();
    let mut s = spec(OperationId(1), &d, p(None, Some(Variant::Welch)));
    s.capability_version = 2;
    let r = validate(&d, &s);
    assert_eq!(r.status, ValidationStatus::Unsupported);
    assert_eq!(r.errors[0].code, TTestCode::CapabilityVersionMismatch);
    let mut s = spec(OperationId(1), &d, p(None, Some(Variant::Welch)));
    s.capability_id = CapabilityId::parse("stats.describe").unwrap();
    assert_eq!(
        validate(&d, &s).errors[0].code,
        TTestCode::UnknownCapability
    );
    let mut s = spec(OperationId(1), &d, p(None, Some(Variant::Welch)));
    s.inputs.clear();
    assert_eq!(validate(&d, &s).errors[0].code, TTestCode::InputCount);
}

#[test]
fn rows_are_left_out_once_each_by_the_first_reason_and_codes_apply_to_the_outcome_only() {
    let d = table(
        &["y", "g"],
        &[
            &["", ""],     // missing_group (and a missing outcome): counted once
            &["2", "c"],   // other_level
            &["", "a"],    // missing_outcome
            &["-99", "a"], // missing_code
            &["1", "a"],
            &["3", "a"],
            &["4", "b"],
            &["6", "b"],
            &["5", "-99"], // the group's "-99" is a level, not a code
        ],
    );
    let params = TTestParams {
        missing_codes: vec![-99.0],
        ..p(Some([1, 2]), Some(Variant::Welch))
    };
    // Levels in code-point order: "-99", "a", "b", "c".
    let r = compute(&d, &params).expect("computes");
    assert_eq!(
        (r.groups[0].label.as_str(), r.groups[1].label.as_str()),
        ("a", "b")
    );
    let counts: Vec<(TTestCode, u64)> = r.exclusions.iter().map(|e| (e.code, e.count)).collect();
    assert_eq!(
        counts,
        [
            (TTestCode::MissingGroup, 1),
            (TTestCode::OtherLevel, 2),
            (TTestCode::MissingOutcome, 1),
            (TTestCode::MissingCode, 1),
        ]
    );
    assert_eq!(r.groups[0].n + r.groups[1].n + 5, d.rows() as u64);
    // The -99 group is a level of its own; with it chosen it has one value.
    let params = TTestParams {
        levels: Some([0, 1]),
        ..params
    };
    let e = compute(&d, &params).unwrap_err();
    assert_eq!(e.code, TTestCode::GroupTooSmall);
    assert_eq!(e.params.get(&ParamKey::Index), Some(&ParamValue::Int(0)));
    assert_eq!(e.params.get(&ParamKey::Count), Some(&ParamValue::Int(1)));
}

#[test]
fn a_group_too_small_after_the_exclusions_is_refused_at_validation() {
    let d = table(
        &["y", "g"],
        &[&["1", "a"], &["", "a"], &["3", "b"], &["4", "b"]],
    );
    let (s, errors, _) = status(&d, p(None, Some(Variant::Student)));
    assert_eq!(s, ValidationStatus::Invalid);
    assert_eq!(errors, [TTestCode::GroupTooSmall]);
}

#[test]
fn both_groups_constant_is_not_computable_and_one_constant_group_is_computed() {
    let both = table(
        &["y", "g"],
        &[&["2", "a"], &["2", "a"], &["5", "b"], &["5", "b"]],
    );
    for v in [Variant::Student, Variant::Welch] {
        assert!(validate(&both, &spec(OperationId(1), &both, p(None, Some(v)))).may_run());
        assert_eq!(
            compute(&both, &p(None, Some(v))).unwrap_err().code,
            TTestCode::BothGroupsConstant
        );
    }
    let one = table(
        &["y", "g"],
        &[
            &["2", "a"],
            &["2", "a"],
            &["2", "a"],
            &["5", "b"],
            &["6", "b"],
            &["8", "b"],
        ],
    );
    for v in [Variant::Student, Variant::Welch] {
        let r = compute(&one, &p(None, Some(v))).expect("one constant group computes");
        assert_eq!(r.groups[0].sd.to_bits(), 0f64.to_bits());
        assert_eq!(r.sd_ratio, Stat::NotComputable(TTestCode::SdRatioUndefined));
        assert!(r.t.is_finite() && r.p.is_finite() && r.cohens_d.is_finite());
    }
}

#[test]
fn the_check_point_runs_once_before_the_arithmetic_and_can_stop_it() {
    let d = two_levels();
    let mut calls = Vec::new();
    let r = run(&d, &p(None, Some(Variant::Welch)), |done| {
        calls.push(done);
        ControlFlow::<()>::Continue(())
    });
    assert!(r.is_ok());
    assert_eq!(calls, [0]);
    let stopped = run(&d, &p(None, Some(Variant::Welch)), |_| {
        ControlFlow::Break("cancel")
    });
    assert_eq!(stopped, Err(Halt::Stopped { reason: "cancel" }));
    // A refused spec never reaches the check point.
    let mut reached = false;
    let refused = run(&d, &p(None, None), |_| {
        reached = true;
        ControlFlow::<()>::Continue(())
    });
    assert!(matches!(refused, Err(Halt::Refused(_))) && !reached);
}

#[test]
fn ten_thousand_and_four_rows_compute_with_df_10002_and_the_interval_from_t_quantile_975() {
    // 10,004 rows: Student's df is 10,002, above stats_dist::LARGE_DF (1e4),
    // where the quantile is the Cornish–Fisher expansion and p is Hill's
    // (Research OS, Phase 8E, decision 275). Refused until 8E. The ingest's
    // 5,000-row cap keeps a real file below it, so a dataset built in memory
    // pins it here: the interval is the mean difference ± q · se, bit for bit,
    // q being `t_quantile_975(10_002.0)`.
    let rows: Vec<[String; 2]> = (0..10_004)
        .map(|i| {
            [
                format!("{}", (i % 7) as f64 + 0.5 * (i % 2) as f64),
                if i % 2 == 0 { "a".into() } else { "b".into() },
            ]
        })
        .collect();
    let rows: Vec<Vec<&str>> = rows
        .iter()
        .map(|r| vec![r[0].as_str(), r[1].as_str()])
        .collect();
    let rows: Vec<&[&str]> = rows.iter().map(|r| r.as_slice()).collect();
    let d = table(&["y", "g"], &rows);
    let r = compute(&d, &p(None, Some(Variant::Student))).expect("computes above df 1e4");
    assert_eq!(r.df, 10_002.0);
    let q = t_quantile_975(10_002.0).expect("the quantile at df 10,002");
    let half = q * r.standard_error;
    assert_eq!(r.ci_lower.to_bits(), (r.mean_difference - half).to_bits());
    assert_eq!(r.ci_upper.to_bits(), (r.mean_difference + half).to_bits());
    assert!(r.p > 0.0 && r.p <= 1.0, "p {}", r.p);
    // 10,002 rows give df 10,000 exactly: the bisection's last df, computed.
    let d = table(&["y", "g"], &rows[..10_002]);
    let r = compute(&d, &p(None, Some(Variant::Student))).expect("computes");
    assert_eq!(r.df, 10_000.0);
}

#[test]
fn overflow_and_underflow_keep_their_own_codes_as_in_the_descriptive_summary() {
    use gaply_core::contracts::codes::Closed;
    // Decision 70: a sum a double cannot hold is `overflow`; squares that fall
    // below the smallest normal double although the values differ are
    // `underflow`. Asserted on the wire words, so the test compiles before
    // the codes exist and fails on what the run returns.
    let over = table(
        &["y", "g"],
        &[&["1e308", "a"], &["1.5e308", "a"], &["1", "b"], &["2", "b"]],
    );
    let under = table(
        &["y", "g"],
        &[
            &["1e-170", "a"],
            &["2e-170", "a"],
            &["3e-170", "a"],
            &["4e-170", "b"],
            &["5e-170", "b"],
        ],
    );
    for v in [Variant::Student, Variant::Welch] {
        let e = compute(&over, &p(None, Some(v))).unwrap_err();
        assert_eq!(e.code.wire(), "stats.ttest.overflow", "{v:?}");
        let e = compute(&under, &p(None, Some(v))).unwrap_err();
        assert_eq!(e.code.wire(), "stats.ttest.underflow", "{v:?}");
    }
}

/// Decision 81: `group_levels` is the index a spec's `levels` names, by
/// construction. A group coded in numbers (1, 2, 10: code-point order puts
/// "10" before "2") and one with an empty cell: for every ordered pair of
/// positions, group 1 of the result is the level at the first position, with
/// that level's count as its n. An unknown column is `None`.
#[test]
fn group_levels_are_the_indices_a_spec_names() {
    let d = table(
        &["y", "g"],
        &[
            &["1", "1"],
            &["2", "1"],
            &["4", "2"],
            &["5", "2"],
            &["7", "10"],
            &["8.5", "10"],
            &["3", ""],
        ],
    );
    let g = Alias(ColumnAlias(2));
    let levels = group_levels(&d, g).expect("the column exists");
    assert_eq!(
        levels,
        vec![
            ("1".to_string(), 2),
            ("10".to_string(), 2),
            ("2".to_string(), 2)
        ],
        "code-point order, the empty cell not a level"
    );
    for i in 0..3u32 {
        for j in 0..3u32 {
            if i == j {
                continue;
            }
            let r = compute(&d, &p(Some([i, j]), Some(Variant::Welch))).expect("computes");
            assert_eq!(r.groups[0].label, levels[i as usize].0, "[{i}, {j}]");
            assert_eq!(r.groups[1].label, levels[j as usize].0, "[{i}, {j}]");
            assert_eq!(r.groups[0].n, levels[i as usize].1);
            assert_eq!(r.groups[0].level, i);
        }
    }
    assert_eq!(group_levels(&d, Alias(ColumnAlias(9))), None);
}

/// Decision 94 (the Phase 4 evaluation's F4): decision 87's limit of 50
/// levels holds wherever a spec can be built, not only in the picker and in
/// `stats_ttest_group_levels`. A group with 51 levels is refused by
/// `validate` and by `run` with `too_many_levels {count, limit}`, even when
/// the spec names two levels that each hold two rows and would compute; a
/// group with exactly 50 is not refused for its count.
#[test]
fn a_group_with_more_than_50_levels_is_refused_by_validate_and_run() {
    fn levels(k: usize) -> Dataset {
        let rows: Vec<Vec<String>> = (0..k)
            .flat_map(|i| {
                [
                    vec![format!("{}", i), format!("L{i:02}")],
                    vec![format!("{}.5", i + 7), format!("L{i:02}")],
                ]
            })
            .collect();
        let refs: Vec<Vec<&str>> = rows
            .iter()
            .map(|r| r.iter().map(String::as_str).collect())
            .collect();
        let slices: Vec<&[&str]> = refs.iter().map(Vec::as_slice).collect();
        table(&["y", "g"], &slices)
    }
    let welch = Some(Variant::Welch);

    let wide = levels(51);
    let r = validate(&wide, &spec(OperationId(1), &wide, p(Some([0, 1]), welch)));
    assert_eq!(r.status, ValidationStatus::Invalid, "{r:?}");
    let e = r
        .errors
        .iter()
        .find(|e| e.code == TTestCode::TooManyLevels)
        .unwrap_or_else(|| panic!("too_many_levels among the errors: {:?}", r.errors));
    assert_eq!(e.params.get(&ParamKey::Count), Some(&ParamValue::Int(51)));
    assert_eq!(e.params.get(&ParamKey::Limit), Some(&ParamValue::Int(50)));
    // No levels named: still refused, not asked which two.
    let (s, errors, questions) = status(&wide, p(None, welch));
    assert_eq!(s, ValidationStatus::Invalid);
    assert!(errors.contains(&TTestCode::TooManyLevels), "{errors:?}");
    assert!(
        !questions.contains(&TTestCode::LevelsNotChosen),
        "{questions:?}"
    );
    // `run` refuses with the same code before its check point.
    let mut reached = false;
    let refused = run(&wide, &p(Some([0, 1]), welch), |_| {
        reached = true;
        ControlFlow::<()>::Continue(())
    });
    assert!(
        matches!(&refused, Err(Halt::Refused(d)) if d.code == TTestCode::TooManyLevels),
        "{refused:?}"
    );
    assert!(!reached, "a refused spec never reaches the check point");

    // Exactly 50: the two named levels compute.
    let fifty = levels(50);
    let (s, errors, _) = status(&fifty, p(Some([0, 1]), welch));
    assert_eq!(s, ValidationStatus::Valid, "{errors:?}");
    assert!(compute(&fifty, &p(Some([0, 1]), welch)).is_ok());
}
