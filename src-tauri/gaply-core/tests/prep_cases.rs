//! **Scale scores' own rules (Research OS, Phase 8M, decision 408), beside
//! the R golden (`prep_r_golden.rs`).**
//!
//! * The recipe's one form: its JSON spelling, unknown fields and
//!   non-canonical bytes refused, and a reading that does not echo a word.
//! * Every refusal with its closed code and numbers, never a name or a value.
//! * Nothing chosen for the user: a statistic or a rule left open needs input.
//! * The build: which rows are scored, the four counts, an empty cell and
//!   never a code.
//! * The caps, the prepared input hash, the CSV's bytes and `Debug`.

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{CapabilityId, Diagnostic, ParamKey, ParamValue, ValidationStatus};
use gaply_core::stats_alpha::ScaleEnds;
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, sha256_hex, Cell, ColumnAlias, ColumnType, DataFingerprint, Dataset, RawTable,
    TableRef,
};
use gaply_core::stats_prep::{
    build, export::prepared_csv, prepared_fingerprint, recipe_bytes, recipe_hash, validate,
    MissingRule, PrepCode, PrepRefusal, Prepared, Recipe, RecipeStep, ScaleScoreStep, ScoreCounts,
    Statistic, CAPABILITY_ID, CAPABILITY_VERSION, ENGINE_VERSION, RECIPE_SCHEMA_VERSION,
};

const CAP: u64 = 5_000_000;
const NAME_SENTINEL: &str = "NAME_SENTINEL_q7";

fn dataset(headers: &[&str], rows: &[&[&str]]) -> Dataset {
    build_dataset(
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter()
                .map(|r| r.iter().map(|c| c.to_string()).collect())
                .collect(),
        ),
        DataFingerprint::of_bytes(b"prep cases", "prep cases parser", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn a(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn step(alias: u32, items: &[u32]) -> ScaleScoreStep {
    ScaleScoreStep {
        alias: a(alias),
        name: "total".to_string(),
        items: items.iter().map(|i| a(*i)).collect(),
        reversed: Vec::new(),
        scale: None,
        statistic: Some(Statistic::Sum),
        rule: Some(MissingRule::EveryItem),
        missing_codes: Vec::new(),
    }
}

fn recipe(columns: u32, steps: Vec<ScaleScoreStep>) -> Recipe {
    Recipe {
        schema_version: RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(CAPABILITY_ID).unwrap(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        source_columns: columns,
        steps: steps.into_iter().map(RecipeStep::ScaleScore).collect(),
    }
}

/// Five rows of three 1-to-5 items, with an empty cell and a code of 99.
fn table() -> Dataset {
    dataset(
        &["id", "q1", "q2", "q3"],
        &[
            &["1", "1", "2", "3"],
            &["2", "5", "", "1"],
            &["3", "", "", "2"],
            &["4", "99", "4", "4"],
            &["5", "", "", ""],
        ],
    )
}

fn base() -> ScaleScoreStep {
    step(5, &[2, 3, 4])
}

fn codes(r: &[Diagnostic<PrepCode>]) -> Vec<PrepCode> {
    r.iter().map(|d| d.code).collect()
}

/// The errors for one step on [`table`]; the report must be Invalid.
fn errors(s: ScaleScoreStep) -> Vec<Diagnostic<PrepCode>> {
    let report = validate(&table(), &recipe(4, vec![s]), CAP);
    assert_eq!(report.status, ValidationStatus::Invalid, "{report:?}");
    report.errors
}

fn int(d: &Diagnostic<PrepCode>, key: ParamKey) -> i64 {
    match d.params.get(&key) {
        Some(ParamValue::Int(v)) => *v,
        other => panic!("{key:?}: {other:?}"),
    }
}

fn prepared(d: &Dataset, r: &Recipe) -> Prepared {
    build(d, r, CAP).unwrap_or_else(|e| panic!("{e:?}"))
}

fn cells(p: &Prepared, alias: u32) -> Vec<Option<f64>> {
    p.dataset
        .column_cells(ColumnAlias(alias))
        .expect("column")
        .iter()
        .map(|c| match c {
            Cell::Number(v) => Some(*v),
            Cell::Missing => None,
            Cell::Text(_) => panic!("a score holds no text"),
        })
        .collect()
}

/* ---- the recipe's one form --------------------------------------------- */

#[test]
fn the_recipes_spelling_is_snake_case_externally_tagged_and_camel_case_fields() {
    let mut s = base();
    s.reversed = vec![a(3)];
    s.scale = Some(ScaleEnds { min: 1.0, max: 5.0 });
    s.statistic = Some(Statistic::Mean);
    s.rule = Some(MissingRule::AtLeast(2));
    s.missing_codes = vec![-99.0];
    let bytes = recipe_bytes(&recipe(4, vec![s]));
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "{\"schemaVersion\":1,\"capabilityId\":\"stats.prepare.scale_score\",\
         \"capabilityVersion\":1,\"engineVersion\":1,\"sourceColumns\":4,\"steps\":[\
         {\"kind\":\"scale_score\",\"alias\":\"C5\",\"name\":\"total\",\
         \"items\":[\"C2\",\"C3\",\"C4\"],\"reversed\":[\"C3\"],\
         \"scale\":{\"min\":1.0,\"max\":5.0},\"statistic\":\"mean\",\
         \"rule\":{\"at_least\":2},\"missingCodes\":[-99.0]}]}"
    );
    let every = recipe_bytes(&recipe(4, vec![base()]));
    let every = String::from_utf8(every).unwrap();
    assert!(every.contains("\"statistic\":\"sum\""), "{every}");
    assert!(every.contains("\"rule\":\"every_item\""), "{every}");
}

#[test]
fn canonical_bytes_read_back_to_the_same_recipe_and_nothing_else_is_read() {
    let r = recipe(4, vec![base()]);
    let bytes = recipe_bytes(&r);
    assert_eq!(Recipe::from_canonical_bytes(&bytes).unwrap(), r);

    let text = String::from_utf8(bytes.clone()).unwrap();
    // An unknown field at each level, a different spacing, a different order
    // and a field left out: each refused, with one closed code.
    let unknown_top = text.replacen("{\"schemaVersion\"", "{\"x\":1,\"schemaVersion\"", 1);
    let unknown_step = text.replacen("\"alias\"", "\"x\":1,\"alias\"", 1);
    let spaced = text.replacen("\"sourceColumns\":4", "\"sourceColumns\": 4", 1);
    let no_scale = text.replacen("\"scale\":null,", "", 1);
    for (what, bad) in [
        ("unknown top", unknown_top),
        ("unknown step", unknown_step),
        ("spaced", spaced),
        ("no scale", no_scale),
    ] {
        assert_ne!(bad, text, "{what}: the edit changed nothing");
        assert_eq!(
            Recipe::from_canonical_bytes(bad.as_bytes()).unwrap_err(),
            PrepCode::RecipeUnreadable,
            "{what}"
        );
    }
    assert_eq!(
        Recipe::from_canonical_bytes(NAME_SENTINEL.as_bytes()).unwrap_err(),
        PrepCode::RecipeUnreadable
    );
}

#[test]
fn every_code_has_the_namespace_and_the_four_counts_are_closed_words() {
    assert!(PrepCode::ALL
        .iter()
        .all(|c| c.wire().starts_with("stats.prep.")));
    let counts = ScoreCounts {
        column: a(5),
        scored_from_every_item: 1,
        scored_from_fewer: 2,
        not_scored_too_few: 3,
        not_scored_overflow: 4,
    };
    assert_eq!(
        counts.by_code(),
        [
            (PrepCode::ScoredFromEveryItem, 1),
            (PrepCode::ScoredFromFewer, 2),
            (PrepCode::NotScoredTooFew, 3),
            (PrepCode::NotScoredOverflow, 4),
        ]
    );
}

/* ---- the recipe's fields ----------------------------------------------- */

#[test]
fn a_recipe_for_another_capability_version_or_schema_is_unsupported() {
    let d = table();
    let mut r = recipe(4, vec![base()]);
    r.capability_id = CapabilityId::parse("stats.describe").unwrap();
    r.capability_version = 2;
    r.engine_version = 2;
    r.schema_version = 2;
    let report = validate(&d, &r, CAP);
    assert_eq!(report.status, ValidationStatus::Unsupported);
    assert_eq!(
        codes(&report.errors)[..4],
        [
            PrepCode::UnknownCapability,
            PrepCode::CapabilityVersionMismatch,
            PrepCode::EngineVersionMismatch,
            PrepCode::SchemaVersionMismatch,
        ]
    );
}

#[test]
fn source_columns_must_be_the_files_and_a_recipe_needs_a_step() {
    let e = validate(&table(), &recipe(3, vec![step(4, &[2, 3])]), CAP).errors;
    assert_eq!(e[0].code, PrepCode::SourceColumnsMismatch);
    assert_eq!(int(&e[0], ParamKey::Expected), 4);
    assert_eq!(int(&e[0], ParamKey::Found), 3);
    let e = validate(&table(), &recipe(4, vec![]), CAP).errors;
    assert_eq!(codes(&e), [PrepCode::NoSteps]);
}

/// Decision 408: each step's alias is exactly C(k + j), in the recipe's order.
#[test]
fn each_alias_is_the_next_after_the_files_columns_in_the_recipes_order() {
    let d = table();
    let mut second = step(6, &[2, 3]);
    second.name = "other".to_string();
    assert_eq!(
        validate(&d, &recipe(4, vec![base(), second.clone()]), CAP).status,
        ValidationStatus::Valid
    );
    let swapped = validate(&d, &recipe(4, vec![second, base()]), CAP);
    let wrong: Vec<_> = swapped
        .errors
        .iter()
        .filter(|e| e.code == PrepCode::AliasNotNext)
        .collect();
    assert_eq!(wrong.len(), 2, "{:?}", swapped.errors);
    assert_eq!(int(wrong[0], ParamKey::Expected), 5);
    assert_eq!(int(wrong[0], ParamKey::Found), 6);
    let e = errors(step(7, &[2, 3]));
    assert_eq!(codes(&e), [PrepCode::AliasNotNext]);
}

/// A prepared table is not a source: the recipe is applied to the file's
/// table (ADR-011 question 13).
#[test]
fn a_prepared_table_is_refused_as_a_source() {
    let d = table();
    let r = recipe(4, vec![base()]);
    let once = prepared(&d, &r);
    let mut again = step(6, &[2, 3]);
    again.name = "again".to_string();
    let e = validate(&once.dataset, &recipe(4, vec![again]), CAP).errors;
    assert!(
        codes(&e).contains(&PrepCode::SourceAlreadyPrepared),
        "{e:?}"
    );
}

/* ---- the name ---------------------------------------------------------- */

fn named(name: &str) -> ScaleScoreStep {
    let mut s = base();
    s.name = name.to_string();
    s
}

#[test]
fn a_name_is_taken_as_typed_trimmed_one_to_128_characters_and_visible() {
    let ok = "é".repeat(128);
    for good in ["t", "Total score", ok.as_str(), "c3", "C", "C3x", "C03"] {
        assert_eq!(
            validate(&table(), &recipe(4, vec![named(good)]), CAP).status,
            ValidationStatus::Valid,
            "{good}"
        );
    }
    let long = "é".repeat(129);
    for (bad, code) in [
        ("", PrepCode::NameLength),
        (long.as_str(), PrepCode::NameLength),
        (" total", PrepCode::NameNotTrimmed),
        ("total ", PrepCode::NameNotTrimmed),
        ("\ttotal", PrepCode::NameNotTrimmed),
        ("to\ntal", PrepCode::NameHasInvisibleCharacter),
        ("to\u{7}tal", PrepCode::NameHasInvisibleCharacter),
        ("to\u{202E}tal", PrepCode::NameHasInvisibleCharacter),
        ("to\u{200B}tal", PrepCode::NameHasInvisibleCharacter),
        ("to\u{FEFF}tal", PrepCode::NameHasInvisibleCharacter),
    ] {
        let e = errors(named(bad));
        assert_eq!(codes(&e), [code], "{bad:?}");
        assert_eq!(int(&e[0], ParamKey::Index), 0);
    }
}

#[test]
fn a_name_may_not_be_a_file_header_another_scores_name_or_an_alias_word() {
    // Compared after trimming both sides, as build_dataset trims headers.
    let d = dataset(&["id", " q1 ", "q2"], &[&["1", "1", "2"], &["2", "3", "4"]]);
    let mut s = step(4, &[2, 3]);
    s.name = "q1".to_string();
    let e = validate(&d, &recipe(3, vec![s.clone()]), CAP).errors;
    assert_eq!(codes(&e), [PrepCode::NameEqualsFileHeader]);
    assert_eq!(int(&e[0], ParamKey::Column), 2);

    s.name = "dup".to_string();
    let mut t = step(5, &[2, 3]);
    t.name = "dup".to_string();
    let e = validate(&d, &recipe(3, vec![s, t]), CAP).errors;
    assert_eq!(codes(&e), [PrepCode::NameEqualsOtherScore]);
    assert_eq!(int(&e[0], ParamKey::Index), 1);
    assert_eq!(int(&e[0], ParamKey::From), 0);

    for word in ["C1", "C3", "C5", "C128", "C999999"] {
        assert_eq!(
            codes(&errors(named(word))),
            [PrepCode::NameIsAnAliasWord],
            "{word}"
        );
    }
}

/* ---- the items --------------------------------------------------------- */

#[test]
fn items_are_two_or_more_of_the_files_numeric_columns_in_order_and_once_each() {
    assert_eq!(
        codes(&errors(step(5, &[2]))),
        [PrepCode::TooFewItems],
        "one item"
    );
    let e = errors(step(5, &[3, 2]));
    assert_eq!(codes(&e), [PrepCode::ItemsOutOfOrder]);
    assert_eq!(int(&e[0], ParamKey::Column), 2);
    assert_eq!(
        codes(&errors(step(5, &[2, 2, 3]))),
        [PrepCode::ItemRepeated]
    );
    // Another score, the score itself, and no such column: the file's only.
    for outside in [5u32, 6, 40] {
        let e = errors(step(5, &[2, outside]));
        assert_eq!(codes(&e), [PrepCode::ItemNotAFileColumn], "C{outside}");
        assert_eq!(int(&e[0], ParamKey::Column), i64::from(outside));
    }
}

#[test]
fn an_item_holding_text_or_a_non_finite_cell_or_nothing_is_refused_with_its_count() {
    let d = dataset(
        &["mixed", "inf", "empty", "ok"],
        &[
            &["1", "inf", "", "1"],
            &["x", "2", "", "2"],
            &["y", "-inf", "", "3"],
        ],
    );
    let mut s = step(5, &[1, 4]);
    let e = validate(&d, &recipe(4, vec![s.clone()]), CAP).errors;
    assert_eq!(codes(&e), [PrepCode::ItemHoldsText]);
    assert_eq!(
        (int(&e[0], ParamKey::Column), int(&e[0], ParamKey::Count)),
        (1, 2)
    );
    s.items = vec![a(2), a(4)];
    let e = validate(&d, &recipe(4, vec![s.clone()]), CAP).errors;
    assert_eq!(codes(&e), [PrepCode::ItemNonFinite]);
    assert_eq!(
        (int(&e[0], ParamKey::Column), int(&e[0], ParamKey::Count)),
        (2, 2)
    );
    s.items = vec![a(3), a(4)];
    let e = validate(&d, &recipe(4, vec![s]), CAP).errors;
    assert_eq!(codes(&e), [PrepCode::ItemNotNumeric]);
    assert_eq!(int(&e[0], ParamKey::Column), 3);
}

/* ---- reversal ---------------------------------------------------------- */

fn reversed(items: &[u32], scale: Option<(f64, f64)>) -> ScaleScoreStep {
    let mut s = base();
    s.reversed = items.iter().map(|i| a(*i)).collect();
    s.scale = scale.map(|(min, max)| ScaleEnds { min, max });
    s.missing_codes = vec![99.0];
    s
}

#[test]
fn the_reversed_items_and_the_scale_have_one_form() {
    let e = errors(reversed(&[1], Some((1.0, 5.0))));
    assert_eq!(codes(&e), [PrepCode::ReversedNotAnItem]);
    assert_eq!(int(&e[0], ParamKey::Column), 1);
    assert!(
        codes(&errors(reversed(&[3, 2], Some((1.0, 5.0))))).contains(&PrepCode::ReversedOutOfOrder)
    );
    assert!(
        codes(&errors(reversed(&[2, 2], Some((1.0, 5.0))))).contains(&PrepCode::ReversedRepeated)
    );
    assert_eq!(
        codes(&errors(reversed(&[2], None))),
        [PrepCode::ScaleMissing]
    );
    assert_eq!(
        codes(&errors(reversed(&[], Some((1.0, 5.0))))),
        [PrepCode::ScaleWithoutReversed]
    );
    for ends in [
        (f64::NAN, 5.0),
        (1.0, f64::INFINITY),
        (f64::NEG_INFINITY, 5.0),
    ] {
        assert_eq!(
            codes(&errors(reversed(&[2], Some(ends)))),
            [PrepCode::ScaleNotFinite]
        );
    }
    for ends in [(5.0, 1.0), (3.0, 3.0)] {
        assert_eq!(
            codes(&errors(reversed(&[2], Some(ends)))),
            [PrepCode::ScaleEndsOutOfOrder]
        );
    }
}

#[test]
fn a_reversed_value_outside_the_ends_is_refused_with_its_column_and_count_codes_removed() {
    // q1 holds 1, 5, 99 and two empties. Ticked, the 99 does not count.
    let ok = validate(
        &table(),
        &recipe(4, vec![reversed(&[2], Some((1.0, 5.0)))]),
        CAP,
    );
    assert_eq!(ok.status, ValidationStatus::Valid, "{ok:?}");
    // Unticked it does.
    let mut s = reversed(&[2], Some((1.0, 5.0)));
    s.missing_codes.clear();
    let e = errors(s);
    assert_eq!(codes(&e), [PrepCode::ReversedOutsideScale]);
    assert_eq!(
        (int(&e[0], ParamKey::Column), int(&e[0], ParamKey::Count)),
        (2, 1)
    );
    // An end nearer than a value: q3 holds 1, 2, 3, 4.
    let e = errors(reversed(&[4], Some((2.0, 3.0))));
    assert_eq!(codes(&e), [PrepCode::ReversedOutsideScale]);
    assert_eq!(
        (int(&e[0], ParamKey::Column), int(&e[0], ParamKey::Count)),
        (4, 2)
    );
}

/* ---- the rule, the codes, and what is left open ------------------------ */

#[test]
fn at_least_runs_from_one_to_one_below_the_items() {
    for n in [1, 2] {
        let mut s = base();
        s.rule = Some(MissingRule::AtLeast(n));
        assert_eq!(
            validate(&table(), &recipe(4, vec![s]), CAP).errors.len(),
            0,
            "{n}"
        );
    }
    for n in [0, 3, 4] {
        let mut s = base();
        s.rule = Some(MissingRule::AtLeast(n));
        let e = errors(s);
        assert_eq!(codes(&e), [PrepCode::AtLeastOutOfRange], "{n}");
        assert_eq!(int(&e[0], ParamKey::Found), i64::from(n));
        assert_eq!(int(&e[0], ParamKey::Limit), 2);
    }
}

#[test]
fn missing_codes_are_finite_ascending_and_listed_once() {
    for (list, code, index) in [
        (vec![f64::NAN], PrepCode::MissingCodeNotFinite, 0),
        (vec![1.0, f64::INFINITY], PrepCode::MissingCodeNotFinite, 1),
        (vec![1.0, 1.0], PrepCode::MissingCodeRepeated, 1),
        (vec![2.0, 1.0], PrepCode::MissingCodesOutOfOrder, 1),
    ] {
        let mut s = base();
        s.missing_codes = list;
        let e = errors(s);
        assert_eq!(codes(&e), [code]);
        assert_eq!(int(&e[0], ParamKey::Index), index);
    }
}

/// Decision 408: no default. A statistic or rule left open needs input, as
/// the t-test's variant does, and nothing is built.
#[test]
fn a_statistic_or_rule_left_open_needs_input_and_builds_nothing() {
    let d = table();
    let mut s = base();
    s.statistic = None;
    let report = validate(&d, &recipe(4, vec![s.clone()]), CAP);
    assert_eq!(report.status, ValidationStatus::NeedsUserInput);
    assert_eq!(
        codes(&report.missing_information),
        [PrepCode::StatisticNotChosen]
    );
    assert_eq!(int(&report.missing_information[0], ParamKey::Index), 0);
    s.rule = None;
    let report = validate(&d, &recipe(4, vec![s.clone()]), CAP);
    assert_eq!(
        codes(&report.missing_information),
        [PrepCode::StatisticNotChosen, PrepCode::RuleNotChosen]
    );
    match build(&d, &recipe(4, vec![s]), CAP) {
        Err(PrepRefusal::Report(r)) => assert_eq!(r.status, ValidationStatus::NeedsUserInput),
        other => panic!("{:?}", other.map(|p| p.counts)),
    }
}

#[test]
fn a_sum_under_at_least_carries_a_note_and_a_mean_does_not() {
    let mut s = base();
    s.rule = Some(MissingRule::AtLeast(2));
    let report = validate(&table(), &recipe(4, vec![s.clone()]), CAP);
    assert_eq!(report.status, ValidationStatus::ValidWithWarnings);
    assert_eq!(codes(&report.warnings), [PrepCode::SumOverFewerItems]);
    s.statistic = Some(Statistic::Mean);
    let report = validate(&table(), &recipe(4, vec![s]), CAP);
    assert_eq!(report.status, ValidationStatus::Valid);
}

/* ---- the build --------------------------------------------------------- */

#[test]
fn every_item_scores_only_a_complete_row_and_counts_the_rest() {
    let d = table();
    let mut s = base();
    s.missing_codes = vec![99.0];
    let p = prepared(&d, &recipe(4, vec![s]));
    assert_eq!(cells(&p, 5), [Some(6.0), None, None, None, None]);
    assert_eq!(
        p.counts,
        [ScoreCounts {
            column: a(5),
            scored_from_every_item: 1,
            scored_from_fewer: 0,
            not_scored_too_few: 4,
            not_scored_overflow: 0,
        }]
    );
}

#[test]
fn at_least_scores_exactly_n_values_and_not_one_fewer() {
    let d = table();
    let mut s = base();
    s.missing_codes = vec![99.0];
    s.rule = Some(MissingRule::AtLeast(2));
    s.statistic = Some(Statistic::Mean);
    let p = prepared(&d, &recipe(4, vec![s.clone()]));
    // Rows hold 3, 2, 1, 2 (99 ticked) and 0 values.
    assert_eq!(cells(&p, 5), [Some(2.0), Some(3.0), None, Some(4.0), None]);
    assert_eq!(p.counts[0].scored_from_every_item, 1);
    assert_eq!(p.counts[0].scored_from_fewer, 2);
    assert_eq!(p.counts[0].not_scored_too_few, 2);
    s.rule = Some(MissingRule::AtLeast(1));
    s.statistic = Some(Statistic::Sum);
    let p = prepared(&d, &recipe(4, vec![s]));
    assert_eq!(
        cells(&p, 5),
        [Some(6.0), Some(6.0), Some(2.0), Some(8.0), None],
        "no rescaling, and a row with none has no score"
    );
}

#[test]
fn an_unticked_code_is_a_value_and_a_ticked_one_is_not() {
    let d = table();
    let mut s = base();
    s.rule = Some(MissingRule::AtLeast(1));
    let p = prepared(&d, &recipe(4, vec![s.clone()]));
    assert_eq!(cells(&p, 5)[3], Some(107.0));
    s.missing_codes = vec![99.0];
    let p = prepared(&d, &recipe(4, vec![s]));
    assert_eq!(cells(&p, 5)[3], Some(8.0));
}

#[test]
fn a_reversal_is_on_the_ends_given_and_a_code_outside_them_is_not_reversed() {
    let d = table();
    let mut s = reversed(&[2], Some((1.0, 5.0)));
    s.rule = Some(MissingRule::AtLeast(1));
    let p = prepared(&d, &recipe(4, vec![s]));
    // q1 reversed: 1 -> 5, 5 -> 1; the 99 is a code.
    assert_eq!(
        cells(&p, 5),
        [Some(10.0), Some(2.0), Some(2.0), Some(8.0), None]
    );
}

/// A score's column is Numeric and typed by the constructor; it never has a
/// level list, and its header is the alias word, never the typed name.
#[test]
fn the_typed_name_never_reaches_the_schema() {
    let d = table();
    let mut s = base();
    s.name = NAME_SENTINEL.to_string();
    let p = prepared(&d, &recipe(4, vec![s]));
    let c = p.dataset.schema.column(ColumnAlias(5)).unwrap();
    assert_eq!(c.header, "C5");
    assert_eq!(c.column_type, ColumnType::Numeric);
    assert!(c.levels.is_none() && !c.is_group());
    for column in &p.dataset.schema.columns {
        assert!(!column.header.contains(NAME_SENTINEL));
    }
    assert!(!format!("{:?}", p.dataset).contains(NAME_SENTINEL));
}

#[test]
fn a_total_that_overflows_is_an_empty_cell_and_is_counted() {
    let big = "1.7e308";
    let d = dataset(
        &["a", "b"],
        &[&[big, big], &["1", "2"], &["-1.7e308", "-1.7e308"]],
    );
    for statistic in [Statistic::Sum, Statistic::Mean] {
        let mut s = step(3, &[1, 2]);
        s.statistic = Some(statistic);
        let p = prepared(&d, &recipe(2, vec![s]));
        let want = match statistic {
            Statistic::Sum => 3.0,
            Statistic::Mean => 1.5,
        };
        assert_eq!(cells(&p, 3), [None, Some(want), None], "{statistic:?}");
        assert_eq!(p.counts[0].scored_from_every_item, 1);
        assert_eq!(p.counts[0].not_scored_overflow, 2);
        assert_eq!(p.counts[0].not_scored_too_few, 0);
    }
}

/// min + max can overflow on ends a double holds: the row is counted as an
/// overflow, never written infinite.
#[test]
fn a_reversal_that_overflows_is_an_empty_cell_too() {
    let d = dataset(&["a", "b"], &[&["1.6e308", "1"]]);
    let mut s = step(3, &[1, 2]);
    s.reversed = vec![a(1)];
    s.scale = Some(ScaleEnds {
        min: 1.5e308,
        max: 1.7e308,
    });
    s.rule = Some(MissingRule::AtLeast(1));
    match build(&d, &recipe(2, vec![s]), CAP) {
        Err(PrepRefusal::NoRowScored { column, counts }) => {
            assert_eq!(column, a(3));
            assert_eq!(counts.not_scored_overflow, 1);
        }
        other => panic!("{:?}", other.map(|p| p.counts)),
    }
}

#[test]
fn a_score_no_row_can_have_is_refused_with_its_counts() {
    let d = dataset(&["a", "b"], &[&["1", ""], &["", "2"], &["", ""]]);
    match build(&d, &recipe(2, vec![step(3, &[1, 2])]), CAP) {
        Err(PrepRefusal::NoRowScored { column, counts }) => {
            assert_eq!(column, a(3));
            assert_eq!(
                counts,
                ScoreCounts {
                    column: a(3),
                    scored_from_every_item: 0,
                    scored_from_fewer: 0,
                    not_scored_too_few: 3,
                    not_scored_overflow: 0,
                }
            );
        }
        other => panic!("{:?}", other.map(|p| p.counts)),
    }
}

#[test]
fn two_scores_are_each_built_and_counted_in_the_recipes_order() {
    let d = table();
    let mut first = base();
    first.missing_codes = vec![99.0];
    let mut second = step(6, &[3, 4]);
    second.name = "second".to_string();
    second.statistic = Some(Statistic::Mean);
    let p = prepared(&d, &recipe(4, vec![first, second]));
    assert_eq!(p.dataset.schema.columns.len(), 6);
    assert_eq!(
        p.counts.iter().map(|c| c.column).collect::<Vec<_>>(),
        [a(5), a(6)]
    );
    assert_eq!(cells(&p, 6), [Some(2.5), None, None, Some(4.0), None]);
}

/* ---- the caps ---------------------------------------------------------- */

fn wide(columns: usize) -> Dataset {
    let headers: Vec<String> = (0..columns).map(|i| format!("h{i}")).collect();
    let rows = vec![
        (0..columns).map(|i| i.to_string()).collect(),
        (0..columns).map(|i| (i + 1).to_string()).collect(),
    ];
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(b"wide", "prep cases parser", TableRef::FirstTable),
    )
    .unwrap()
}

/// Decision 408: 2 to 127 items. 128 items needs a 128-column file, and a
/// score's column would make 129: TooWide, one code, not a second rule.
#[test]
fn one_hundred_and_twenty_seven_items_take_a_score_and_128_are_too_wide() {
    let items: Vec<u32> = (1..=127).collect();
    let d = wide(127);
    let p = prepared(&d, &recipe(127, vec![step(128, &items)]));
    assert_eq!(p.dataset.schema.columns.len(), 128);

    let items: Vec<u32> = (1..=128).collect();
    let d = wide(128);
    let r = recipe(128, vec![step(129, &items)]);
    let report = validate(&d, &r, CAP);
    assert_eq!(codes(&report.errors), [PrepCode::TooWide]);
    assert_eq!(int(&report.errors[0], ParamKey::Found), 129);
    assert_eq!(int(&report.errors[0], ParamKey::Limit), 128);
    match build(&d, &r, CAP) {
        Err(PrepRefusal::Report(r)) => assert_eq!(codes(&r.errors), [PrepCode::TooWide]),
        other => panic!("{:?}", other.map(|p| p.counts)),
    }
    // Two items of a full file: the same refusal.
    let report = validate(&d, &recipe(128, vec![step(129, &[1, 2])]), CAP);
    assert_eq!(codes(&report.errors), [PrepCode::TooWide]);
}

#[test]
fn the_cells_cap_is_the_callers_and_counts_the_score() {
    let d = wide(3);
    let r = recipe(3, vec![step(4, &[1, 2])]);
    // Two rows of three columns and one score: 8 cells.
    assert_eq!(validate(&d, &r, 8).status, ValidationStatus::Valid);
    let report = validate(&d, &r, 7);
    assert_eq!(codes(&report.errors), [PrepCode::CellsOverCap]);
    assert_eq!(int(&report.errors[0], ParamKey::Found), 8);
    assert_eq!(int(&report.errors[0], ParamKey::Limit), 7);
    assert!(build(&d, &r, 7).is_err());
    assert!(build(&d, &r, 8).is_ok());
}

/* ---- the prepared input hash ------------------------------------------- */

#[test]
fn the_prepared_input_hash_is_over_the_originals_hash_and_the_recipes() {
    let d = table();
    let r = recipe(4, vec![base()]);
    let source = &d.schema.fingerprint;
    let want = sha256_hex(
        format!(
            "gaply.prepared.v1\n{}\n{}",
            sha256_hex(b"prep cases"),
            sha256_hex(&recipe_bytes(&r))
        )
        .as_bytes(),
    );
    assert_eq!(recipe_hash(&r), sha256_hex(&recipe_bytes(&r)));
    let fp = prepared_fingerprint(source, &r);
    assert_eq!(fp.sha256_hex(), want);
    assert_eq!(fp.parser_version(), source.parser_version());
    assert_eq!(fp.table(), source.table());
    assert_eq!(prepared(&d, &r).dataset.schema.fingerprint, fp);
}

#[test]
fn two_recipes_are_two_inputs_and_so_are_two_files() {
    let d = table();
    let r = recipe(4, vec![base()]);
    let renamed = recipe(4, vec![named("another")]);
    let source = &d.schema.fingerprint;
    assert_ne!(
        prepared_fingerprint(source, &r).sha256_hex(),
        prepared_fingerprint(source, &renamed).sha256_hex()
    );
    let other = DataFingerprint::of_bytes(b"other file", "prep cases parser", TableRef::FirstTable);
    assert_ne!(
        prepared_fingerprint(source, &r).sha256_hex(),
        prepared_fingerprint(&other, &r).sha256_hex()
    );
}

/* ---- the CSV ----------------------------------------------------------- */

#[test]
fn the_csv_holds_the_headers_the_typed_names_and_every_row_in_crlf() {
    let d = dataset(
        &["id", "=q1", "q\"2"],
        &[
            &["-1.50", "-2", "x"],
            &["2", "", "=SUM(A1)"],
            &["3", "1", "y"],
        ],
    );
    let d2 = d.clone();
    let mut s = step(4, &[1, 2]);
    s.name = "score, \"the\" total".to_string();
    s.rule = Some(MissingRule::AtLeast(1));
    let p = prepared(&d2, &recipe(3, vec![s]));
    assert_eq!(
        String::from_utf8(prepared_csv(&p)).unwrap(),
        "\"id\",\"'=q1\",\"q\"\"2\",\"score, \"\"the\"\" total\"\r\n\
         -1.5,-2,\"x\",-3.5\r\n\
         2,,\"'=SUM(A1)\",2\r\n\
         3,1,\"y\",4\r\n"
    );
}

/// ADR-011 Part 8: a number never passes through `text()`, whose guard
/// includes `-`, so a negative score stays a number a spreadsheet reads.
#[test]
fn a_negative_score_and_a_negative_item_are_written_unquoted() {
    let d = dataset(&["a", "b"], &[&["-1", "-2.25"]]);
    let p = prepared(&d, &recipe(2, vec![step(3, &[1, 2])]));
    let csv = String::from_utf8(prepared_csv(&p)).unwrap();
    assert_eq!(csv, "\"a\",\"b\",\"total\"\r\n-1,-2.25,-3.25\r\n");
    assert!(!csv.contains('\''));
}

/// A name that a spreadsheet would run is guarded, the same rule as any
/// header.
#[test]
fn a_typed_name_that_starts_like_a_formula_is_guarded_in_the_header() {
    let d = dataset(&["a", "b"], &[&["1", "2"]]);
    let mut s = step(3, &[1, 2]);
    s.name = "=cmd()".to_string();
    let p = prepared(&d, &recipe(2, vec![s]));
    let csv = String::from_utf8(prepared_csv(&p)).unwrap();
    assert!(csv.starts_with("\"a\",\"b\",\"'=cmd()\"\r\n"), "{csv}");
}

#[test]
fn every_number_is_written_at_all_its_digits() {
    let tiny = 1.0e-300f64;
    let third = 1.0f64 / 3.0;
    let cell = format!("{third:?}");
    let d = dataset(&["a", "b"], &[&[cell.as_str(), "1e-300"]]);
    let p = prepared(&d, &recipe(2, vec![step(3, &[1, 2])]));
    let csv = String::from_utf8(prepared_csv(&p)).unwrap();
    let row = csv.lines().nth(1).unwrap();
    let parts: Vec<f64> = row.split(',').map(|x| x.parse().unwrap()).collect();
    assert_eq!(parts[0].to_bits(), third.to_bits());
    assert_eq!(parts[1].to_bits(), tiny.to_bits());
    assert_eq!(parts[2].to_bits(), (third + tiny).to_bits());
}

/* ---- Debug ------------------------------------------------------------- */

#[test]
fn debug_prints_counts_and_never_a_name_or_a_value() {
    let d = dataset(&["a", "b"], &[&["1.25", "2.5"], &["3", "4"]]);
    let mut s = step(3, &[1, 2]);
    s.name = NAME_SENTINEL.to_string();
    s.reversed = vec![a(1)];
    s.scale = Some(ScaleEnds {
        min: 0.125,
        max: 8192.5,
    });
    s.missing_codes = vec![-777.5];
    let r = recipe(2, vec![s.clone()]);
    let p = prepared(&d, &r);
    for text in [
        format!("{r:?}"),
        format!("{:?}", r.steps[0]),
        format!("{s:?}"),
        format!("{p:?}"),
    ] {
        for hidden in [NAME_SENTINEL, "0.125", "8192.5", "777.5", "1.25", "8191"] {
            assert!(!text.contains(hidden), "{hidden} in {text}");
        }
    }
    assert!(format!("{s:?}").contains("items: 2"), "{s:?}");
}

/// A refusal is closed words and numbers: the name and its characters never
/// appear, in `Debug` or on the wire.
#[test]
fn a_refusal_holds_no_name() {
    let mut s = step(5, &[2, 3]);
    s.name = format!(" {NAME_SENTINEL}");
    let report = validate(&table(), &recipe(4, vec![s]), CAP);
    assert_eq!(report.status, ValidationStatus::Invalid);
    assert!(!format!("{report:?}").contains(NAME_SENTINEL));
    assert!(!serde_json::to_string(&report)
        .unwrap()
        .contains(NAME_SENTINEL));
}

/// Applying one recipe twice gives the same table, bit for bit.
#[test]
fn ten_builds_give_the_same_columns() {
    let d = table();
    let mut s = reversed(&[2], Some((1.0, 5.0)));
    s.rule = Some(MissingRule::AtLeast(2));
    s.statistic = Some(Statistic::Mean);
    let r = recipe(4, vec![s]);
    let first = prepared(&d, &r);
    for _ in 0..10 {
        let again = prepared(&d, &r);
        assert_eq!(again.dataset, first.dataset);
        assert_eq!(prepared_csv(&again), prepared_csv(&first));
    }
}

/// The source is not changed: its bytes, its schema and its fingerprint.
#[test]
fn the_source_table_is_unchanged() {
    let d = table();
    let before = d.clone();
    let _ = prepared(&d, &recipe(4, vec![base()]));
    assert_eq!(d, before);
}

/// Decision 416 (8M's question 4): -0.0 and 0.0 are one number with two
/// spellings, so they would hash as two recipes. Refused as a scale end and as
/// a missing code, by a code of its own; a screen sends "-0" as 0.
#[test]
fn negative_zero_is_refused_as_a_scale_end_and_as_a_missing_code() {
    let wires = |s: ScaleScoreStep| -> Vec<String> {
        errors(s)
            .iter()
            .map(|d| d.code.wire().to_string())
            .collect()
    };
    let negative_zero = "stats.prep.negative_zero";
    let mut min = base();
    min.reversed = vec![a(2)];
    min.scale = Some(ScaleEnds {
        min: -0.0,
        max: 100.0,
    });
    assert_eq!(wires(min), [negative_zero], "the lower end");

    let mut max = base();
    max.reversed = vec![a(2)];
    max.scale = Some(ScaleEnds {
        min: -100.0,
        max: -0.0,
    });
    let got = wires(max);
    assert!(
        got.iter().any(|c| c == negative_zero),
        "the upper end: {got:?}"
    );

    let mut code = base();
    code.missing_codes = vec![-0.0, 99.0];
    assert_eq!(wires(code), [negative_zero], "a missing code");

    // Plain zero is a number like any other, as an end and as a code.
    let mut zero = base();
    zero.reversed = vec![a(2)];
    zero.scale = Some(ScaleEnds {
        min: 0.0,
        max: 100.0,
    });
    zero.missing_codes = vec![0.0, 99.0];
    let report = validate(&table(), &recipe(4, vec![zero]), CAP);
    assert_eq!(report.status, ValidationStatus::Valid, "{report:?}");
}
