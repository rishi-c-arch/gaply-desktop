//! **Cronbach's alpha's own rules (Research OS, Phase 8I, decision 367),
//! beside the R golden (`alpha_r_golden.rs`).**
//!
//! * One choice, one spec: items, reversed items and codes repeated or out
//!   of order, a reversed item not among the items, a scale without a
//!   reversed item or a reversed item without a scale, and a scale whose
//!   ends are not finite or not in order, each refused with its code.
//! * A reversed item's value outside the scale, in any row read, refused
//!   with its column and count; a ticked code there is not counted.
//! * Text, non-finite cells, too few rows, and a candidate missing code's
//!   warning, as the correlations treat them.
//! * The check points: 1 + k + k(k − 1)/2, in order, and a stop at any of
//!   them returns no result.
//! * Rows used and the exclusions add up to the rows read, in the manifest
//!   too; ten runs give the same bytes; the result holds no header or cell.

use std::ops::ControlFlow;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{Arch, OperationId, Os, Platform, ValidationStatus};
use gaply_core::stats_alpha::{
    check_points, compute, manifest, run, spec, AlphaCode, AlphaParams, Halt, ScaleEnds, Stat,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

fn dataset(headers: &[&str], rows: &[&[&str]]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter()
                .map(|r| r.iter().map(|c| c.to_string()).collect())
                .collect(),
        ),
        DataFingerprint::of_bytes(b"alpha cases", "alpha cases", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn a(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

/// Five 1-to-5 items over eight rows, the third worded the other way.
fn likert() -> Dataset {
    dataset(
        &["q1", "q2", "q3", "q4", "q5"],
        &[
            &["1", "2", "5", "1", "2"],
            &["2", "2", "4", "2", "1"],
            &["3", "3", "3", "3", "3"],
            &["4", "5", "2", "4", "4"],
            &["5", "4", "1", "5", "5"],
            &["2", "1", "4", "3", "2"],
            &["4", "4", "2", "4", "3"],
            &["3", "2", "3", "2", "3"],
        ],
    )
}

fn params(items: &[u32]) -> AlphaParams {
    AlphaParams {
        items: items.iter().map(|n| a(*n)).collect(),
        reversed: Vec::new(),
        scale: None,
        missing_codes: Vec::new(),
    }
}

fn refusal(d: &Dataset, p: &AlphaParams) -> (String, String) {
    let e = compute(d, p).expect_err("refused");
    (e.code.wire().to_string(), format!("{:?}", e.params))
}

fn first_code(d: &Dataset, p: AlphaParams) -> String {
    let report = gaply_core::stats_alpha::validate(d, &spec(OperationId(1), d, p));
    assert_eq!(report.status, ValidationStatus::Invalid, "{report:?}");
    report.errors[0].code.wire().to_string()
}

#[test]
fn one_choice_one_spec_each_refused_with_its_code() {
    let d = likert();
    let ends = Some(ScaleEnds { min: 1.0, max: 5.0 });
    let with = |f: &dyn Fn(&mut AlphaParams)| {
        let mut p = params(&[1, 2, 3, 4, 5]);
        f(&mut p);
        first_code(&d, p)
    };
    let cases: Vec<(&str, String)> = vec![
        ("stats.alpha.too_few_items", with(&|p| p.items = vec![a(1)])),
        (
            "stats.alpha.item_repeated",
            with(&|p| p.items = vec![a(1), a(1), a(2)]),
        ),
        (
            "stats.alpha.items_out_of_order",
            with(&|p| p.items = vec![a(2), a(1)]),
        ),
        (
            "stats.alpha.reversed_repeated",
            with(&|p| {
                p.reversed = vec![a(3), a(3)];
                p.scale = ends;
            }),
        ),
        (
            "stats.alpha.reversed_out_of_order",
            with(&|p| {
                p.reversed = vec![a(3), a(1)];
                p.scale = ends;
            }),
        ),
        (
            "stats.alpha.reversed_not_an_item",
            with(&|p| {
                p.items = vec![a(1), a(2)];
                p.reversed = vec![a(3)];
                p.scale = ends;
            }),
        ),
        (
            "stats.alpha.scale_without_reversed",
            with(&|p| p.scale = ends),
        ),
        (
            "stats.alpha.scale_missing",
            with(&|p| p.reversed = vec![a(3)]),
        ),
        (
            "stats.alpha.scale_not_finite",
            with(&|p| {
                p.reversed = vec![a(3)];
                p.scale = Some(ScaleEnds {
                    min: 1.0,
                    max: f64::INFINITY,
                });
            }),
        ),
        (
            "stats.alpha.scale_ends_out_of_order",
            with(&|p| {
                p.reversed = vec![a(3)];
                p.scale = Some(ScaleEnds { min: 5.0, max: 5.0 });
            }),
        ),
        (
            "stats.alpha.missing_code_not_finite",
            with(&|p| p.missing_codes = vec![f64::NAN]),
        ),
        (
            "stats.alpha.missing_code_repeated",
            with(&|p| p.missing_codes = vec![-9.0, -9.0]),
        ),
        (
            "stats.alpha.missing_codes_out_of_order",
            with(&|p| p.missing_codes = vec![9.0, -9.0]),
        ),
        (
            "stats.alpha.unknown_column",
            with(&|p| p.items = vec![a(1), a(9)]),
        ),
    ];
    for (want, got) in cases {
        assert_eq!(got, want);
    }
}

#[test]
fn a_reversed_value_outside_the_scale_is_refused_with_its_column_and_count() {
    let d = dataset(
        &["q1", "q2", "q3"],
        &[
            &["1", "7", "2"],
            &["2", "4", "3"],
            &["3", "-9", "3"],
            &["4", "0", "5"],
            &["5", "1", "4"],
        ],
    );
    let mut p = params(&[1, 2, 3]);
    p.reversed = vec![a(2)];
    p.scale = Some(ScaleEnds { min: 1.0, max: 5.0 });
    p.missing_codes = vec![-9.0];
    // 7 and 0 are outside; -9 is a ticked code and is not counted.
    let (code, params) = refusal(&d, &p);
    assert_eq!(code, "stats.alpha.reversed_outside_scale");
    assert!(
        params.contains("Column: Int(2)") && params.contains("Count: Int(2)"),
        "{params}"
    );
    // Unticked, -9 is a value outside the scale too.
    p.missing_codes = Vec::new();
    assert!(refusal(&d, &p).1.contains("Count: Int(3)"));
    // The same values on an item not reversed are read as they are.
    p.reversed = Vec::new();
    p.scale = None;
    assert!(compute(&d, &p).is_ok());
}

#[test]
fn text_non_finite_and_too_few_rows_are_refused_and_a_candidate_code_warned_of() {
    let text = dataset(
        &["x", "y"],
        &[&["1", "a"], &["2", "3"], &["3", "4"], &["4", "5"]],
    );
    assert_eq!(
        refusal(&text, &params(&[1, 2])).0,
        "stats.alpha.item_holds_text"
    );
    let few = dataset(
        &["x", "y"],
        &[&["1", ""], &["2", "3"], &["3", "4"], &["", "5"]],
    );
    let (code, p) = refusal(&few, &params(&[1, 2]));
    assert_eq!(
        (code.as_str(), p.contains("Count: Int(2)")),
        ("stats.alpha.too_few_complete_rows", true)
    );
    let coded = dataset(
        &["x", "y"],
        &[
            &["1", "2"],
            &["2", "-99"],
            &["3", "4"],
            &["4", "5"],
            &["5", "5"],
        ],
    );
    let r =
        gaply_core::stats_alpha::validate(&coded, &spec(OperationId(1), &coded, params(&[1, 2])));
    assert_eq!(r.status, ValidationStatus::ValidWithWarnings, "{r:?}");
    assert_eq!(r.warnings[0].code, AlphaCode::SuspectMissingCode);
}

#[test]
fn the_check_points_are_one_per_item_and_per_pair_after_the_first() {
    let d = likert();
    let p = params(&[1, 2, 3, 4, 5]);
    assert_eq!(check_points(&p), 1 + 5 + 10);
    let mut seen = Vec::new();
    run(&d, &p, |i| {
        seen.push(i);
        ControlFlow::<()>::Continue(())
    })
    .expect("ran");
    assert_eq!(seen, (0..16).collect::<Vec<_>>());
    for stop in 0..16 {
        let got = run(&d, &p, |i| {
            if i == stop {
                ControlFlow::Break(stop)
            } else {
                ControlFlow::Continue(())
            }
        });
        assert_eq!(
            got.err(),
            Some(Halt::Stopped { reason: stop }),
            "stop at {stop}"
        );
    }
}

#[test]
fn a_negative_alpha_carries_its_note_and_reversing_removes_it() {
    let d = likert();
    let p = params(&[1, 2, 3, 4, 5]);
    let unreversed = compute(&d, &p).unwrap();
    let Stat::Value(alpha) = unreversed.alpha else {
        panic!("{:?}", unreversed.alpha)
    };
    let mut rev = p.clone();
    rev.reversed = vec![a(3)];
    rev.scale = Some(ScaleEnds { min: 1.0, max: 5.0 });
    let reversed = compute(&d, &rev).unwrap();
    let Stat::Value(alpha_rev) = reversed.alpha else {
        panic!()
    };
    assert!(alpha_rev > alpha, "{alpha_rev} > {alpha}");
    assert!(reversed.notes.is_empty());
    assert!(reversed.items[2].reversed && !reversed.items[1].reversed);
    // Reversed on the scale's ends: q3's mean is 6 minus its read mean.
    let (Stat::Value(m), Stat::Value(m_rev)) = (unreversed.items[2].mean, reversed.items[2].mean)
    else {
        panic!()
    };
    assert_eq!(m_rev, 6.0 - m);
    // Two items x and 6 - x, unreversed: alpha negative where defined.
    let neg = dataset(
        &["x", "y", "z"],
        &[
            &["1", "5", "2"],
            &["2", "3", "2"],
            &["4", "2", "3"],
            &["5", "1", "2"],
        ],
    );
    let r = compute(&neg, &params(&[1, 2, 3])).unwrap();
    assert!(
        matches!(r.alpha, Stat::Value(v) if v < 0.0),
        "{:?}",
        r.alpha
    );
    assert_eq!(r.notes, vec![AlphaCode::NegativeAlpha]);
}

#[test]
fn rows_used_and_exclusions_add_up_in_the_result_and_the_manifest() {
    let d = dataset(
        &["x", "y", "z"],
        &[
            &["1", "2", "3"],
            &["", "2", ""],
            &["2", "-9", "4"],
            &["3", "4", ""],
            &["4", "5", "5"],
            &["5", "5", "6"],
        ],
    );
    let mut p = params(&[1, 2, 3]);
    p.missing_codes = vec![-9.0];
    let r = compute(&d, &p).unwrap();
    assert_eq!(r.rows_used, 3);
    let excluded: Vec<(String, u64)> = r
        .exclusions
        .iter()
        .map(|e| (e.code.wire().to_string(), e.count))
        .collect();
    assert_eq!(
        excluded,
        [
            ("stats.alpha.missing_value".to_string(), 1),
            ("stats.alpha.missing_code".to_string(), 1),
            ("stats.alpha.missing_value".to_string(), 1),
        ]
    );
    let s = spec(OperationId(1), &d, p);
    let m = manifest(
        &s,
        &d,
        &r,
        12,
        Platform {
            os: Os::Macos,
            arch: Arch::Aarch64,
        },
    );
    assert_eq!(m.rows_read, 6);
    assert_eq!(
        m.rows_used + m.exclusions.iter().map(|e| e.count).sum::<u64>(),
        m.rows_read
    );
}

#[test]
fn ten_runs_give_the_same_bytes_and_the_result_holds_no_header_or_cell() {
    let d = dataset(
        &[
            "SENTINEL_HEADER_a1",
            "SENTINEL_HEADER_b2",
            "SENTINEL_HEADER_c3",
        ],
        &[
            &["1.5", "2", "3"],
            &["2", "2.75", "4"],
            &["3", "4", "4.125"],
            &["4", "5", "5"],
        ],
    );
    let p = params(&[1, 2, 3]);
    let first = compute(&d, &p).unwrap().canonical_bytes();
    for _ in 0..9 {
        assert_eq!(compute(&d, &p).unwrap().canonical_bytes(), first);
    }
    let text = String::from_utf8(first).unwrap();
    assert!(!text.contains("SENTINEL"), "{text}");
    // Positive control: the cells' own numbers are not in it either.
    for cell in ["2.75", "4.125"] {
        assert!(!text.contains(cell), "{cell} in {text}");
    }
}

#[test]
fn the_parameters_have_one_spelling_on_the_wire() {
    let p = AlphaParams {
        items: vec![a(1), a(2)],
        reversed: vec![a(2)],
        scale: Some(ScaleEnds { min: 1.0, max: 5.0 }),
        missing_codes: vec![-9.0],
    };
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(
        json,
        r#"{"items":["C1","C2"],"reversed":["C2"],"scale":{"min":1.0,"max":5.0},"missingCodes":[-9.0]}"#
    );
    assert_eq!(serde_json::from_str::<AlphaParams>(&json).unwrap(), p);
    assert!(
        serde_json::from_str::<AlphaParams>(&json.replace("\"items\"", "\"variables\"")).is_err()
    );
}
