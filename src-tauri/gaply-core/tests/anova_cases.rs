//! **`gaply_core::stats_anova`'s closed outcomes, its check points and its
//! refusals (Research OS, Phase 8G, decisions 312 and 317).** The numbers
//! are `anova_r_golden.rs`'s; this file pins what is decided rather than
//! computed: which levels are included and in what order, which refusals a
//! spec meets, that a run stops at a post hoc pair's check point, that the
//! rows' order changes no bit, and which notes a result carries.

use std::ops::ControlFlow;

use gaply_core::contracts::{OperationId, ParamKey, ParamValue};
use gaply_core::stats_anova::{
    check_points, compute, run, spec, validate, AnovaCode, AnovaParams, Halt, PostHoc,
    PostHocPairs, POST_HOC_MAX_LEVELS,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};

fn dataset(rows: &[(&str, &str)]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            vec!["y".into(), "g".into()],
            rows.iter()
                .map(|(y, g)| vec![y.to_string(), g.to_string()])
                .collect(),
        ),
        DataFingerprint::of_bytes(b"anova cases", "anova cases", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn alias(d: &Dataset, header: &str) -> Alias {
    Alias(
        d.schema
            .columns
            .iter()
            .find(|c| c.header == header)
            .unwrap()
            .alias,
    )
}

fn params(d: &Dataset, levels: Option<Vec<u32>>, post_hoc: Vec<PostHoc>) -> AnovaParams {
    AnovaParams {
        outcome: alias(d, "y"),
        group: alias(d, "g"),
        levels,
        post_hoc,
        missing_codes: Vec::new(),
    }
}

/// Four levels, three values each, unequal spread.
fn four() -> Dataset {
    dataset(&[
        ("1.0", "b"),
        ("2.5", "b"),
        ("1.7", "b"),
        ("4.0", "B"),
        ("3.1", "B"),
        ("5.2", "B"),
        ("2.2", "_x"),
        ("2.9", "_x"),
        ("2.0", "_x"),
        ("6.6", "a"),
        ("7.9", "a"),
        ("5.8", "a"),
    ])
}

fn int(e: &gaply_core::contracts::Diagnostic<AnovaCode>, k: ParamKey) -> i64 {
    match e.params.get(&k) {
        Some(ParamValue::Int(v)) => *v,
        other => panic!("{k:?}: {other:?}"),
    }
}

#[test]
fn the_levels_are_in_code_point_order_and_each_pair_is_the_first_minus_the_second() {
    let d = four();
    let r = compute(&d, &params(&d, None, vec![PostHoc::Bonferroni])).unwrap();
    let labels: Vec<&str> = r.groups.iter().map(|g| g.label.as_str()).collect();
    assert_eq!(
        labels,
        ["B", "_x", "a", "b"],
        "code-point order, never a locale's"
    );
    let PostHocPairs::Computed(pairs) = &r.post_hoc[0].pairs else {
        panic!("computed")
    };
    let order: Vec<(u32, u32)> = pairs.iter().map(|p| (p.first, p.second)).collect();
    assert_eq!(order, [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]);
    for p in pairs {
        let gaply_core::stats_anova::PairOutcome::Computed(s) = p.outcome else {
            panic!("computed")
        };
        let m = |l: u32| r.groups.iter().find(|g| g.level == l).unwrap().mean;
        assert_eq!(s.difference, m(p.first) - m(p.second));
    }
}

#[test]
fn levels_left_out_are_other_level_and_the_table_is_the_included_levels_alone() {
    let d = four();
    let r = compute(&d, &params(&d, Some(vec![3, 0]), vec![])).unwrap();
    let levels: Vec<u32> = r.groups.iter().map(|g| g.level).collect();
    assert_eq!(
        levels,
        [0, 3],
        "named in any order, kept in the level order"
    );
    assert_eq!(r.exclusions.len(), 1);
    assert_eq!(r.exclusions[0].code, AnovaCode::OtherLevel);
    assert_eq!(r.exclusions[0].count, 6);
}

#[test]
fn a_spec_is_refused_for_each_wrong_choice_of_levels_and_for_a_group_of_one() {
    let d = four();
    let first = |levels: Option<Vec<u32>>| {
        let p = params(&d, levels, vec![]);
        let report = validate(&d, &spec(OperationId(1), &d, p.clone()));
        let e = compute(&d, &p).expect_err("refused");
        assert_eq!(report.errors[0].code, e.code, "validate and run agree");
        e
    };
    let e = first(Some(vec![0, 4]));
    assert_eq!(
        (e.code, int(&e, ParamKey::Index), int(&e, ParamKey::Limit)),
        (AnovaCode::LevelOutOfRange, 4, 3)
    );
    let e = first(Some(vec![1, 1, 2]));
    assert_eq!(
        (e.code, int(&e, ParamKey::Index)),
        (AnovaCode::LevelRepeated, 1)
    );
    let e = first(Some(vec![2]));
    assert_eq!(
        (e.code, int(&e, ParamKey::Count)),
        (AnovaCode::TooFewLevels, 1)
    );
    let one = dataset(&[("1", "a"), ("2", "a"), ("3", "b"), ("4", "c"), ("5", "c")]);
    let e = compute(&one, &params(&one, None, vec![])).expect_err("a group of one");
    assert_eq!(
        (e.code, int(&e, ParamKey::Index), int(&e, ParamKey::Count)),
        (AnovaCode::GroupTooSmall, 1, 1)
    );
    let single = dataset(&[("1", "a"), ("2", "a")]);
    let e = compute(&single, &params(&single, None, vec![])).expect_err("one level");
    assert_eq!(e.code, AnovaCode::GroupOneLevel);
}

#[test]
fn a_group_of_more_than_fifty_levels_is_refused() {
    let rows: Vec<(String, String)> = (0..102)
        .map(|i| (format!("{}", i % 7), format!("L{:03}", i / 2)))
        .collect();
    let refs: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let d = dataset(&refs);
    let e = compute(&d, &params(&d, None, vec![])).expect_err("51 levels");
    assert_eq!(
        (e.code, int(&e, ParamKey::Count), int(&e, ParamKey::Limit)),
        (AnovaCode::TooManyLevels, 51, 50)
    );
}

#[test]
fn a_run_stops_at_the_check_point_before_a_post_hoc_pair() {
    let d = four();
    let p = params(&d, None, vec![PostHoc::Tukey, PostHoc::GamesHowell]);
    assert_eq!(check_points(&p, 4), 1 + 2 * 6);
    let mut seen = Vec::new();
    let halted = run(&d, &p, |i| {
        seen.push(i);
        if i == 8 {
            ControlFlow::Break("cancelled")
        } else {
            ControlFlow::Continue(())
        }
    });
    assert_eq!(
        halted,
        Err(Halt::Stopped {
            reason: "cancelled"
        })
    );
    assert_eq!(
        seen,
        (0..=8).collect::<Vec<_>>(),
        "one before the arithmetic, one per pair"
    );
    let mut all = Vec::new();
    run(&d, &p, |i| {
        all.push(i);
        ControlFlow::<()>::Continue(())
    })
    .unwrap();
    assert_eq!(
        all.len(),
        check_points(&p, 4),
        "every check point is called once"
    );
}

#[test]
fn the_rows_order_changes_no_bit_of_the_result() {
    let d = four();
    let mut rows: Vec<(&str, &str)> = vec![
        ("1.0", "b"),
        ("2.5", "b"),
        ("1.7", "b"),
        ("4.0", "B"),
        ("3.1", "B"),
        ("5.2", "B"),
        ("2.2", "_x"),
        ("2.9", "_x"),
        ("2.0", "_x"),
        ("6.6", "a"),
        ("7.9", "a"),
        ("5.8", "a"),
    ];
    rows.reverse();
    rows.swap(2, 7);
    let shuffled = dataset(&rows);
    let all = vec![PostHoc::Tukey, PostHoc::GamesHowell, PostHoc::Bonferroni];
    let a = compute(&d, &params(&d, None, all.clone())).unwrap();
    let b = compute(&shuffled, &params(&shuffled, None, all)).unwrap();
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
}

#[test]
fn the_post_hoc_tests_run_in_one_order_whatever_order_they_are_ticked_in() {
    let d = four();
    let r = compute(
        &d,
        &params(
            &d,
            None,
            vec![PostHoc::Bonferroni, PostHoc::Tukey, PostHoc::Bonferroni],
        ),
    )
    .unwrap();
    let tests: Vec<PostHoc> = r.post_hoc.iter().map(|t| t.test).collect();
    assert_eq!(tests, [PostHoc::Tukey, PostHoc::Bonferroni]);
    let none = compute(&d, &params(&d, None, vec![])).unwrap();
    assert!(
        none.post_hoc.is_empty(),
        "none is ticked by default and none runs"
    );
}

#[test]
fn a_negative_omega_squared_is_shown_as_computed_with_its_note() {
    let d = dataset(&[
        ("5.1", "a"),
        ("4.8", "a"),
        ("6.2", "a"),
        ("5.5", "a"),
        ("4.9", "a"),
        ("5.0", "b"),
        ("5.3", "b"),
        ("6.1", "b"),
        ("4.7", "b"),
        ("5.6", "b"),
        ("5.2", "c"),
        ("4.9", "c"),
        ("5.8", "c"),
        ("5.4", "c"),
        ("5.0", "c"),
    ]);
    let r = compute(&d, &params(&d, None, vec![])).unwrap();
    let gaply_core::stats_anova::Stat::Value(w) = r.omega_squared else {
        panic!("computed")
    };
    assert!(w < 0.0, "omega squared {w}");
    assert_eq!(r.notes, [AnovaCode::OmegaSquaredNegative]);
}

#[test]
fn a_studentized_range_p_below_the_floor_is_shown_as_below_it_with_its_note() {
    let d = dataset(&[
        ("1.00", "a"),
        ("1.01", "a"),
        ("0.99", "a"),
        ("1.02", "a"),
        ("0.98", "a"),
        ("5.00", "b"),
        ("5.01", "b"),
        ("4.99", "b"),
        ("5.02", "b"),
        ("4.98", "b"),
        ("9.00", "c"),
        ("9.01", "c"),
        ("8.99", "c"),
        ("9.02", "c"),
        ("8.98", "c"),
    ]);
    let r = compute(&d, &params(&d, None, vec![PostHoc::Tukey])).unwrap();
    let PostHocPairs::Computed(pairs) = &r.post_hoc[0].pairs else {
        panic!("computed")
    };
    for p in pairs {
        let gaply_core::stats_anova::PairOutcome::Computed(s) = p.outcome else {
            panic!("computed")
        };
        assert_eq!(
            s.p,
            gaply_core::stats_anova::PostHocP::Below(gaply_core::stats_dist::SRANGE_P_FLOOR)
        );
    }
    assert!(r.notes.contains(&AnovaCode::PBelowFloor));
}

#[test]
fn the_post_hoc_cap_is_the_measured_one() {
    // Decision 317: the cap is set from the release measurement on the
    // method page; at 50 the cap is the group's own limit and refuses
    // nothing a group can reach.
    assert!(POST_HOC_MAX_LEVELS <= 50);
}
