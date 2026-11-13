//! **Every method on a scale score (Research OS, Phase 8M-2, decisions 417 to
//! 419; ADR-011 Parts 3, 4, 5 and 7).**
//!
//! Prepared tables here are built by `stats_prep::build`, never by hand.
//!
//! * All twelve methods run on a score (the rank tests the eighth since Phase
//!   8Y-1, decision 604; the related rank tests the ninth since 8Z-1, decision
//!   638, a score one of their columns; the linear regression the tenth since
//!   9A-1, decision 671, scores two of its predictors; the factor analysis the
//!   eleventh since 9B-1, decision 701, a score one of its three items; the
//!   confirmatory factor analysis the twelfth since 9C-1, decision 746, a score
//!   one of its three columns): each manifest carries the preparation,
//!   each result names the score by its alias alone, and each method's numbers
//!   equal its numbers on the same values given as a file's column. That last
//!   comparison is byte for byte on the result, so no gate is involved: the
//!   two tables hold the same doubles, read back through Rust's shortest
//!   round-trip text.
//! * A forged preparation, two recipes of one width, and a spec carried between
//!   a file's table and its prepared table are each refused, in each method.
//! * A score is never a group, a ticked code equal to a score's value refuses
//!   the spec naming the column and the count, and the suspect-code rule does
//!   not apply to a score. Each is checked in each method that has it, beside a
//!   control on the same values as a file's column, which shows the fixture
//!   reaches the rule.

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{
    CapabilityId, ContentHash, Diagnostic, OperationId, ParamKey, ParamValue, Platform,
    Preparation, ValidationReport, ValidationStatus,
};
use gaply_core::stats_alpha::{self, AlphaParams};
use gaply_core::stats_anova::{self, AnovaParams};
use gaply_core::stats_corr::{self, Coefficient, CorrParams, MissingHandling};
use gaply_core::stats_desc::{self, Alias, ColumnRequest, DescribeParams, TreatAs};
use gaply_core::stats_cfa::{self, CfaParams};
use gaply_core::stats_factor::{self, Extraction, FactorParams, Rotation};
use gaply_core::stats_linear_regression::{self, LinearRegressionParams};
use gaply_core::stats_l1::dataset::{
    build_dataset, Cell, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_onesample::{self, OneSampleParams, PairedParams};
use gaply_core::stats_ranks::{self, IndependentRanksParams};
use gaply_core::stats_related_ranks::{self, RelatedRanksParams};
use gaply_core::stats_prep::{
    build, prepared_fingerprint, prepared_input_hash, recipe_hash, MissingRule, Prepared, Recipe,
    RecipeStep, ScaleScoreStep, Statistic, CAPABILITY_ID, CAPABILITY_VERSION, ENGINE_VERSION,
    RECIPE_SCHEMA_VERSION,
};
use gaply_core::stats_ttest::{self, TTestParams, Variant};
use gaply_core::stats_xtab::{self, CrosstabsParams};

const CAP: u64 = 5_000_000;
const NAME_SENTINEL: &str = "NAME_SENTINEL_q7";

const HEADERS: [&str; 7] = ["id", "grp", "lvl", "q1", "q2", "q3", "q4"];
/// Scored by [`recipe`]: C8 is the sum of q1, q2 and q3 under every item, so
/// row 9 has none and row 12 is exactly 99, a candidate missing code far outside
/// the rest. C9 is the mean of q2, q3 and q4 under at least two, so row 8 is
/// the mean of two values.
const ROWS: [[&str; 7]; 14] = [
    ["1", "a", "x", "1", "2", "3", "4"],
    ["2", "b", "y", "2", "3", "4", "5"],
    ["3", "a", "z", "3", "3", "3", "3"],
    ["4", "b", "x", "4", "2", "2", "1"],
    ["5", "a", "y", "5", "1", "2", "3"],
    ["6", "b", "z", "2", "1", "1", "2"],
    ["7", "a", "x", "2", "4", "5", "1"],
    ["8", "b", "y", "3", "5", "4", ""],
    ["9", "a", "z", "", "3", "2", "4"],
    ["10", "b", "x", "4", "5", "5", "3"],
    ["11", "a", "y", "5", "4", "1", "2"],
    ["12", "b", "z", "90", "5", "4", "2"],
    ["13", "a", "x", "3", "2", "1", "5"],
    ["14", "b", "y", "1", "5", "3", "1"],
];

const GRP: u32 = 2;
const LVL: u32 = 3;
const Q2: u32 = 5;
/// A sum score holding the value 99 once, the rule's candidate.
const SUM: u32 = 8;
const MEAN: u32 = 9;
const SUM_NINETY_NINE_ROW: usize = 11;

fn a(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn table(headers: Vec<String>, rows: Vec<Vec<String>>, bytes: &[u8]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(bytes, "prep methods parser", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn source() -> Dataset {
    table(
        HEADERS.iter().map(|h| h.to_string()).collect(),
        ROWS.iter()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect(),
        b"the source file's bytes",
    )
}

fn step(alias: u32, items: &[u32], statistic: Statistic, rule: MissingRule) -> ScaleScoreStep {
    ScaleScoreStep {
        alias: a(alias),
        name: format!("{NAME_SENTINEL}_{alias}"),
        items: items.iter().map(|i| a(*i)).collect(),
        reversed: Vec::new(),
        scale: None,
        statistic: Some(statistic),
        rule: Some(rule),
        missing_codes: Vec::new(),
    }
}

fn recipe() -> Recipe {
    Recipe {
        schema_version: RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(CAPABILITY_ID).unwrap(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        source_columns: 7,
        steps: vec![
            RecipeStep::ScaleScore(step(
                SUM,
                &[4, 5, 6],
                Statistic::Sum,
                MissingRule::EveryItem,
            )),
            RecipeStep::ScaleScore(step(
                MEAN,
                &[5, 6, 7],
                Statistic::Mean,
                MissingRule::AtLeast(2),
            )),
        ],
    }
}

/// The same width and the same aliases as [`recipe`], and other items.
fn other_recipe() -> Recipe {
    let mut r = recipe();
    r.steps[1] = RecipeStep::ScaleScore(step(
        MEAN,
        &[4, 6, 7],
        Statistic::Mean,
        MissingRule::AtLeast(2),
    ));
    r
}

fn prepared_with(d: &Dataset, r: &Recipe) -> Prepared {
    build(d, r, CAP).unwrap_or_else(|e| panic!("{e:?}"))
}

fn prepared() -> Dataset {
    prepared_with(&source(), &recipe()).dataset
}

fn preparation_of(d: &Dataset, r: &Recipe) -> Preparation {
    Preparation {
        source_hash: ContentHash::from_hex(d.schema.fingerprint.sha256_hex()).unwrap(),
        recipe_hash: ContentHash::from_hex(&recipe_hash(r)).unwrap(),
        source_columns: r.source_columns,
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
    }
}

fn preparation() -> Preparation {
    preparation_of(&source(), &recipe())
}

/// The prepared table's scores as columns of a file: the same values through
/// Rust's shortest round-trip text, under the alias words as headers, so a
/// result that prints a header prints the same one.
fn as_a_file() -> Dataset {
    let p = prepared();
    let score = |alias: u32, row: usize| match p.dataset_cell(alias, row) {
        Cell::Number(v) => format!("{v}"),
        Cell::Missing => String::new(),
        Cell::Text(_) => panic!("a score holds no text"),
    };
    let mut headers: Vec<String> = HEADERS.iter().map(|h| h.to_string()).collect();
    headers.extend(["C8".to_string(), "C9".to_string()]);
    let rows = ROWS
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut row: Vec<String> = r.iter().map(|c| c.to_string()).collect();
            row.extend([score(SUM, i), score(MEAN, i)]);
            row
        })
        .collect();
    table(headers, rows, b"the same values as a file")
}

trait CellAt {
    fn dataset_cell(&self, alias: u32, row: usize) -> &Cell;
}
impl CellAt for Dataset {
    fn dataset_cell(&self, alias: u32, row: usize) -> &Cell {
        &self.column_cells(ColumnAlias(alias)).expect("the column")[row]
    }
}

/* ---- each method, one shape ---------------------------------------------- */

/// A validation report in wire words.
struct Report {
    status: ValidationStatus,
    errors: Vec<(String, Option<i64>, Option<i64>)>,
    warnings: Vec<String>,
}

fn report_of<C: Closed + Copy>(r: ValidationReport<C>) -> Report {
    let param = |d: &Diagnostic<C>, key: ParamKey| match d.params.get(&key) {
        Some(ParamValue::Int(v)) => Some(*v),
        _ => None,
    };
    Report {
        status: r.status,
        errors: r
            .errors
            .iter()
            .map(|d| {
                (
                    d.code.wire().to_string(),
                    param(d, ParamKey::Column),
                    param(d, ParamKey::Count),
                )
            })
            .collect(),
        warnings: r
            .warnings
            .iter()
            .map(|d| d.code.wire().to_string())
            .collect(),
    }
}

/// What one run left behind, in wire words and bytes.
struct Run {
    output: Vec<u8>,
    preparation: Option<Preparation>,
    inputs: Vec<ContentHash>,
    parameters_hash: ContentHash,
    spec_and_manifest_json: String,
}

/// A method, over the columns `SUM` and, where it takes two, `MEAN`.
struct Method {
    name: &'static str,
    prefix: &'static str,
    /// A spec on `made_on` with this preparation and codes, validated on `on`.
    validate: fn(&Dataset, Option<Preparation>, &Dataset, &[f64]) -> Report,
    /// A spec on `d` with this preparation, validated, run and recorded.
    run: fn(&Dataset, Option<Preparation>) -> Run,
}

fn describe_params(codes: &[f64]) -> DescribeParams {
    DescribeParams {
        columns: [SUM, MEAN]
            .iter()
            .map(|c| ColumnRequest {
                alias: a(*c),
                treat_as: TreatAs::Numeric,
            })
            .collect(),
        missing_codes: codes.to_vec(),
    }
}
fn ttest_params(codes: &[f64]) -> TTestParams {
    TTestParams {
        outcome: a(SUM),
        group: a(GRP),
        levels: None,
        variant: Some(Variant::Welch),
        missing_codes: codes.to_vec(),
    }
}
fn one_sample_params(codes: &[f64]) -> OneSampleParams {
    OneSampleParams {
        outcome: a(SUM),
        test_value: Some(9.0),
        missing_codes: codes.to_vec(),
    }
}
fn paired_params(codes: &[f64]) -> PairedParams {
    PairedParams {
        first: a(SUM),
        second: a(MEAN),
        missing_codes: codes.to_vec(),
    }
}
fn anova_params(codes: &[f64]) -> AnovaParams {
    AnovaParams {
        outcome: a(SUM),
        group: a(LVL),
        levels: None,
        post_hoc: Vec::new(),
        missing_codes: codes.to_vec(),
    }
}
fn corr_params(codes: &[f64]) -> CorrParams {
    CorrParams {
        variables: vec![a(SUM), a(MEAN)],
        coefficients: vec![
            Coefficient::Pearson,
            Coefficient::Spearman,
            Coefficient::Kendall,
        ],
        missing: MissingHandling::Pairwise,
        missing_codes: codes.to_vec(),
    }
}
fn ranks_params(codes: &[f64]) -> IndependentRanksParams {
    IndependentRanksParams {
        outcome: a(SUM),
        group: a(LVL),
        levels: None,
        missing_codes: codes.to_vec(),
    }
}
fn related_params(codes: &[f64]) -> RelatedRanksParams {
    RelatedRanksParams {
        columns: vec![a(Q2), a(SUM)],
        missing_codes: codes.to_vec(),
    }
}
fn regression_params(codes: &[f64]) -> LinearRegressionParams {
    LinearRegressionParams {
        outcome: a(Q2),
        predictors: vec![a(SUM), a(MEAN)],
        missing_codes: codes.to_vec(),
    }
}
/// Principal components of q2, q4 and the sum score (Research OS, Phase 9B-1,
/// decision 701). The sum is q1 + q2 + q3, so it is no combination of q2 and q4:
/// over the twelve complete rows R 4.6.1 gives the correlation matrix a
/// determinant of 0.69636722132003526 and one eigenvalue, 1.656, above 1
/// (~/gaply-corpus/os/phase9/9b1/logs/prep_det_*.log).
fn factor_params(codes: &[f64]) -> FactorParams {
    FactorParams {
        items: vec![a(Q2), a(7), a(SUM)],
        extraction: Extraction::PrincipalComponents,
        factors: None,
        rotation: Rotation::None,
        iterations: 25,
        convergence: 0.001,
        missing_codes: codes.to_vec(),
    }
}
/// One factor measured by q2, q3 and the sum score (Research OS, Phase 9C-1,
/// decision 746): over the thirteen complete rows they correlate .649, .427 and
/// .298, and lavaan 0.7-3 fits the model with every variance above 0
/// (~/gaply-corpus/os/phase9/9c1/logs/prep_cfa_choice.log). 0 degrees of
/// freedom.
fn cfa_params(codes: &[f64]) -> CfaParams {
    CfaParams {
        indicators: vec![vec![a(Q2), a(6), a(SUM)]],
        correlated_factors: true,
        missing_codes: codes.to_vec(),
    }
}
fn alpha_params(codes: &[f64]) -> AlphaParams {
    AlphaParams {
        items: vec![a(Q2), a(SUM), a(MEAN)],
        reversed: Vec::new(),
        scale: None,
        missing_codes: codes.to_vec(),
    }
}

macro_rules! method {
    ($name:literal, $prefix:literal, $params:ident, $spec:path, $validate:path, $compute:path, $manifest:path) => {
        Method {
            name: $name,
            prefix: $prefix,
            validate: |made_on, preparation, on, codes| {
                let mut spec = $spec(OperationId(1), made_on, $params(codes));
                spec.preparation = preparation;
                report_of($validate(on, &spec))
            },
            run: |d, preparation| {
                let mut spec = $spec(OperationId(1), d, $params(&[]));
                spec.preparation = preparation;
                let report = report_of($validate(d, &spec));
                assert!(report.errors.is_empty(), "{}: {:?}", $name, report.errors);
                let result = $compute(d, &spec.parameters)
                    .unwrap_or_else(|e| panic!("{}: {:?}", $name, e.code.wire()));
                let manifest = $manifest(&spec, d, &result, 0, Platform::current());
                Run {
                    output: result.canonical_bytes(),
                    preparation: manifest.preparation.clone(),
                    inputs: manifest.inputs.clone(),
                    parameters_hash: manifest.parameters_hash.clone(),
                    spec_and_manifest_json: format!(
                        "{}{}",
                        serde_json::to_string(&spec).unwrap(),
                        serde_json::to_string(&manifest).unwrap()
                    ),
                }
            },
        }
    };
}

fn methods() -> [Method; 12] {
    [
        method!(
            "summary",
            "stats.desc.",
            describe_params,
            stats_desc::spec,
            stats_desc::validate,
            stats_desc::describe,
            stats_desc::manifest
        ),
        method!(
            "t-test",
            "stats.ttest.",
            ttest_params,
            stats_ttest::spec,
            stats_ttest::validate,
            stats_ttest::compute,
            stats_ttest::manifest
        ),
        method!(
            "one-sample",
            "stats.onesample.",
            one_sample_params,
            stats_onesample::one_sample_spec,
            stats_onesample::one_sample_validate,
            stats_onesample::one_sample_compute,
            stats_onesample::one_sample_manifest
        ),
        method!(
            "paired",
            "stats.onesample.",
            paired_params,
            stats_onesample::paired_spec,
            stats_onesample::paired_validate,
            stats_onesample::paired_compute,
            stats_onesample::paired_manifest
        ),
        method!(
            "ANOVA",
            "stats.anova.",
            anova_params,
            stats_anova::spec,
            stats_anova::validate,
            stats_anova::compute,
            stats_anova::manifest
        ),
        method!(
            "correlations",
            "stats.corr.",
            corr_params,
            stats_corr::spec,
            stats_corr::validate,
            stats_corr::compute,
            stats_corr::manifest
        ),
        method!(
            "alpha",
            "stats.alpha.",
            alpha_params,
            stats_alpha::spec,
            stats_alpha::validate,
            stats_alpha::compute,
            stats_alpha::manifest
        ),
        method!(
            "rank tests",
            "stats.ranks.",
            ranks_params,
            stats_ranks::spec,
            stats_ranks::validate,
            stats_ranks::compute,
            stats_ranks::manifest
        ),
        method!(
            "related rank tests",
            "stats.related_ranks.",
            related_params,
            stats_related_ranks::spec,
            stats_related_ranks::validate,
            stats_related_ranks::compute,
            stats_related_ranks::manifest
        ),
        method!(
            "linear regression",
            "stats.linear_regression.",
            regression_params,
            stats_linear_regression::spec,
            stats_linear_regression::validate,
            stats_linear_regression::compute,
            stats_linear_regression::manifest
        ),
        method!(
            "factor analysis",
            "stats.factor.",
            factor_params,
            stats_factor::spec,
            stats_factor::validate,
            stats_factor::compute,
            stats_factor::manifest
        ),
        method!(
            "confirmatory factor analysis",
            "stats.cfa.",
            cfa_params,
            stats_cfa::spec,
            stats_cfa::validate,
            stats_cfa::compute,
            stats_cfa::manifest
        ),
    ]
}

fn words(errors: &[(String, Option<i64>, Option<i64>)]) -> Vec<&str> {
    errors.iter().map(|e| e.0.as_str()).collect()
}

/* ---- all twelve on a score --------------------------------------------------- */

#[test]
fn the_fixture_has_the_score_values_the_tests_below_rely_on() {
    let p = prepared();
    assert!(
        matches!(p.dataset_cell(SUM, 8), Cell::Missing),
        "row 9 has no sum"
    );
    assert_eq!(
        p.dataset_cell(SUM, SUM_NINETY_NINE_ROW),
        &Cell::Number(99.0)
    );
    assert_eq!(p.dataset_cell(MEAN, 7), &Cell::Number(4.5));
    assert_eq!(p.schema.columns.len(), 9);
}

#[test]
fn all_twelve_methods_run_on_a_score_and_their_manifests_carry_the_preparation() {
    let d = prepared();
    let hash = ContentHash::from_hex(d.schema.fingerprint.sha256_hex()).unwrap();
    for m in methods() {
        let run = (m.run)(&d, Some(preparation()));
        assert_eq!(run.preparation, Some(preparation()), "{}", m.name);
        assert_eq!(run.inputs, [hash.clone()], "{}", m.name);
    }
}

#[test]
fn a_run_on_a_files_own_table_writes_no_preparation_key() {
    let d = as_a_file();
    for m in methods() {
        let run = (m.run)(&d, None);
        assert_eq!(run.preparation, None, "{}", m.name);
        assert!(
            !run.spec_and_manifest_json.contains("preparation"),
            "{}: the key is written when absent",
            m.name
        );
    }
}

#[test]
fn each_result_names_a_score_by_its_alias_and_never_by_its_typed_name() {
    let d = prepared();
    for m in methods() {
        let run = (m.run)(&d, Some(preparation()));
        let output = String::from_utf8(run.output).unwrap();
        assert!(!output.contains(NAME_SENTINEL), "{}: {output}", m.name);
        assert!(
            !run.spec_and_manifest_json.contains(NAME_SENTINEL),
            "{}: {}",
            m.name,
            run.spec_and_manifest_json
        );
    }
    // The one result that prints a header prints the alias word.
    let result = stats_desc::describe(&d, &describe_params(&[])).unwrap();
    let json = String::from_utf8(result.canonical_bytes()).unwrap();
    assert!(json.contains("\"header\":\"C8\""), "{json}");
    assert!(json.contains("\"header\":\"C9\""), "{json}");
}

#[test]
fn each_methods_numbers_on_a_score_equal_its_numbers_on_the_same_values_as_a_files_column() {
    let (score, file) = (prepared(), as_a_file());
    for m in methods() {
        let on_score = (m.run)(&score, Some(preparation()));
        let on_file = (m.run)(&file, None);
        assert_eq!(
            String::from_utf8(on_score.output).unwrap(),
            String::from_utf8(on_file.output).unwrap(),
            "{}",
            m.name
        );
        assert_eq!(
            on_score.parameters_hash, on_file.parameters_hash,
            "{}",
            m.name
        );
    }
}

/* ---- the contract's check -------------------------------------------------- */

#[test]
fn the_prepared_fingerprint_is_the_hash_level_functions_answer() {
    let (d, r) = (source(), recipe());
    assert_eq!(
        prepared_fingerprint(&d.schema.fingerprint, &r).sha256_hex(),
        prepared_input_hash(d.schema.fingerprint.sha256_hex(), &recipe_hash(&r))
    );
}

/// A correct spec but for one field of its preparation.
#[test]
fn a_forged_preparation_is_refused_in_each_method_and_nothing_else_is_said() {
    let d = prepared();
    let other_hash = ContentHash::of_bytes(b"something else");
    let forgeries: [(&str, Box<dyn Fn(&mut Preparation)>); 5] = [
        (
            "source hash",
            Box::new({
                let h = other_hash.clone();
                move |p| p.source_hash = h.clone()
            }),
        ),
        (
            "recipe hash",
            Box::new({
                let h = other_hash.clone();
                move |p| p.recipe_hash = h.clone()
            }),
        ),
        ("source columns", Box::new(|p| p.source_columns += 1)),
        (
            "capability version",
            Box::new(|p| p.capability_version += 1),
        ),
        ("engine version", Box::new(|p| p.engine_version += 1)),
    ];
    for m in methods() {
        let want = format!("{}preparation_mismatch", m.prefix);
        let good = (m.validate)(&d, Some(preparation()), &d, &[]);
        assert!(good.errors.is_empty(), "{}: {:?}", m.name, good.errors);
        for (what, forge) in &forgeries {
            let mut forged = preparation();
            forge(&mut forged);
            let r = (m.validate)(&d, Some(forged), &d, &[]);
            assert_eq!(r.status, ValidationStatus::Invalid, "{}: {what}", m.name);
            assert_eq!(words(&r.errors), [want.as_str()], "{}: {what}", m.name);
        }
        let absent = (m.validate)(&d, None, &d, &[]);
        assert_eq!(words(&absent.errors), [want.as_str()], "{}: absent", m.name);
    }
}

/// `build`'s two prepared tables, of one width and the same aliases.
#[test]
fn two_recipes_of_one_width_are_refused_for_each_other() {
    let s = source();
    let (r1, r2) = (recipe(), other_recipe());
    let (d1, d2) = (
        prepared_with(&s, &r1).dataset,
        prepared_with(&s, &r2).dataset,
    );
    assert_eq!(d1.schema.columns.len(), d2.schema.columns.len());
    for m in methods() {
        let mismatch = format!("{}preparation_mismatch", m.prefix);
        let input = format!("{}input_mismatch", m.prefix);
        // A spec made on one, validated on the other.
        let carried = (m.validate)(&d1, Some(preparation_of(&s, &r1)), &d2, &[]);
        let got = words(&carried.errors);
        assert!(got.contains(&input.as_str()), "{}: {got:?}", m.name);
        assert!(got.contains(&mismatch.as_str()), "{}: {got:?}", m.name);
        // The right input, with the other recipe's preparation: only the
        // hash can tell them apart.
        let forged = (m.validate)(&d2, Some(preparation_of(&s, &r1)), &d2, &[]);
        assert_eq!(words(&forged.errors), [mismatch.as_str()], "{}", m.name);
    }
}

#[test]
fn a_spec_made_on_the_original_is_refused_on_the_prepared_table_and_the_reverse() {
    let (s, d) = (source(), prepared());
    for m in methods() {
        let mismatch = format!("{}preparation_mismatch", m.prefix);
        let input = format!("{}input_mismatch", m.prefix);
        let on_prepared = (m.validate)(&s, None, &d, &[]);
        let got = words(&on_prepared.errors);
        assert!(got.contains(&input.as_str()), "{}: {got:?}", m.name);
        assert!(got.contains(&mismatch.as_str()), "{}: {got:?}", m.name);

        let on_original = (m.validate)(&d, Some(preparation()), &s, &[]);
        let got = words(&on_original.errors);
        assert_eq!(on_original.status, ValidationStatus::Invalid, "{}", m.name);
        assert!(got.contains(&input.as_str()), "{}: {got:?}", m.name);
        assert!(got.contains(&mismatch.as_str()), "{}: {got:?}", m.name);
    }
}

/* ---- a score is never a group ---------------------------------------------- */

#[test]
fn a_score_is_refused_as_the_t_tests_group() {
    let d = prepared();
    let mut params = ttest_params(&[]);
    params.outcome = a(Q2);
    params.group = a(SUM);
    params.levels = Some([0, 1]);
    let mut spec = stats_ttest::spec(OperationId(1), &d, params.clone());
    spec.preparation = Some(preparation());
    let r = report_of(stats_ttest::validate(&d, &spec));
    assert_eq!(
        r.errors,
        [("stats.ttest.score_as_group".to_string(), Some(8), None)]
    );
    let refused = stats_ttest::compute(&d, &params).unwrap_err();
    assert_eq!(refused.code.wire(), "stats.ttest.score_as_group");
}

#[test]
fn a_score_is_refused_as_the_anovas_group() {
    let d = prepared();
    let mut params = anova_params(&[]);
    params.outcome = a(Q2);
    params.group = a(SUM);
    let mut spec = stats_anova::spec(OperationId(1), &d, params.clone());
    spec.preparation = Some(preparation());
    let r = report_of(stats_anova::validate(&d, &spec));
    assert_eq!(
        r.errors,
        [("stats.anova.score_as_group".to_string(), Some(8), None)]
    );
    let refused = stats_anova::compute(&d, &params).unwrap_err();
    assert_eq!(refused.code.wire(), "stats.anova.score_as_group");
}

/// Phase 8Y-1 (decision 602): the rank tests take a score as their outcome
/// (in `methods()`), never as their group.
#[test]
fn a_score_is_refused_as_the_rank_tests_group() {
    let d = prepared();
    let mut params = ranks_params(&[]);
    params.outcome = a(Q2);
    params.group = a(SUM);
    let mut spec = stats_ranks::spec(OperationId(1), &d, params.clone());
    spec.preparation = Some(preparation());
    let r = report_of(stats_ranks::validate(&d, &spec));
    assert_eq!(
        r.errors,
        [("stats.ranks.score_as_group".to_string(), Some(8), None)]
    );
    let refused = stats_ranks::compute(&d, &params).unwrap_err();
    assert_eq!(refused.code.wire(), "stats.ranks.score_as_group");
}

#[test]
fn a_score_is_refused_as_the_summarys_categorical_column() {
    let d = prepared();
    let params = DescribeParams {
        columns: vec![ColumnRequest {
            alias: a(SUM),
            treat_as: TreatAs::Categorical,
        }],
        missing_codes: Vec::new(),
    };
    let mut spec = stats_desc::spec(OperationId(1), &d, params.clone());
    spec.preparation = Some(preparation());
    let r = report_of(stats_desc::validate(&d, &spec));
    assert_eq!(
        r.errors,
        [("stats.desc.score_as_categorical".to_string(), Some(8), None)]
    );
    let refused = stats_desc::describe(&d, &params).unwrap_err();
    assert_eq!(refused.code.wire(), "stats.desc.score_as_categorical");
}

/// The same numbers as a file's column are a group like any other: it is the
/// column's origin that refuses, not its values.
#[test]
fn the_same_values_as_a_files_column_may_be_a_group() {
    let d = as_a_file();
    let mut t = ttest_params(&[]);
    (t.outcome, t.group, t.levels) = (a(Q2), a(SUM), Some([0, 1]));
    let r = report_of(stats_ttest::validate(
        &d,
        &stats_ttest::spec(OperationId(1), &d, t),
    ));
    assert!(
        !words(&r.errors).contains(&"stats.ttest.score_as_group"),
        "{:?}",
        r.errors
    );
    let mut an = anova_params(&[]);
    (an.outcome, an.group) = (a(Q2), a(SUM));
    let r = report_of(stats_anova::validate(
        &d,
        &stats_anova::spec(OperationId(1), &d, an),
    ));
    assert!(
        !words(&r.errors).contains(&"stats.anova.score_as_group"),
        "{:?}",
        r.errors
    );
    let mut rk = ranks_params(&[]);
    (rk.outcome, rk.group) = (a(Q2), a(SUM));
    let r = report_of(stats_ranks::validate(
        &d,
        &stats_ranks::spec(OperationId(1), &d, rk),
    ));
    assert!(
        !words(&r.errors).contains(&"stats.ranks.score_as_group"),
        "{:?}",
        r.errors
    );
    let params = DescribeParams {
        columns: vec![ColumnRequest {
            alias: a(SUM),
            treat_as: TreatAs::Categorical,
        }],
        missing_codes: Vec::new(),
    };
    let r = report_of(stats_desc::validate(
        &d,
        &stats_desc::spec(OperationId(1), &d, params),
    ));
    assert!(r.errors.is_empty(), "{:?}", r.errors);
}

/* ---- a ticked code equal to a score's value --------------------------------- */

#[test]
fn a_ticked_code_equal_to_a_scores_value_refuses_the_spec_naming_the_column_and_the_count() {
    let d = prepared();
    for m in methods() {
        let r = (m.validate)(&d, Some(preparation()), &d, &[99.0]);
        assert_eq!(r.status, ValidationStatus::Invalid, "{}", m.name);
        assert_eq!(
            r.errors,
            [(
                format!("{}code_equals_score", m.prefix),
                Some(i64::from(SUM)),
                Some(1)
            )],
            "{}",
            m.name
        );
    }
}

/// The independent review's finding (decision 636): two scores a code collides
/// with are named in the order the user chose the columns, not by alias.
#[test]
fn the_related_rank_tests_name_colliding_scores_in_the_list_order() {
    let d = prepared();
    // 9 is in C8 alone, on three rows; 3 is in C9 alone, on four.
    let mut s = stats_related_ranks::spec(
        OperationId(1),
        &d,
        RelatedRanksParams {
            columns: vec![a(MEAN), a(SUM)],
            missing_codes: vec![3.0, 9.0],
        },
    );
    s.preparation = Some(preparation());
    let r = report_of(stats_related_ranks::validate(&d, &s));
    assert_eq!(
        r.errors,
        [
            (
                "stats.related_ranks.code_equals_score".to_string(),
                Some(i64::from(MEAN)),
                Some(4)
            ),
            (
                "stats.related_ranks.code_equals_score".to_string(),
                Some(i64::from(SUM)),
                Some(3)
            ),
        ]
    );
}

#[test]
fn every_score_a_code_collides_with_is_named_and_a_code_on_no_score_is_a_code() {
    let d = prepared();
    // 9 is in C8 alone, on three rows; 3 is in C9 alone, on four.
    let r = report_of(stats_desc::validate(&d, &{
        let mut s = stats_desc::spec(OperationId(1), &d, describe_params(&[3.0, 9.0]));
        s.preparation = Some(preparation());
        s
    }));
    assert_eq!(
        r.errors,
        [
            ("stats.desc.code_equals_score".to_string(), Some(8), Some(3)),
            ("stats.desc.code_equals_score".to_string(), Some(9), Some(4)),
        ]
    );
    // A code no chosen score holds is a code, and leaves the spec good.
    for m in methods() {
        let r = (m.validate)(&d, Some(preparation()), &d, &[-1.0]);
        assert!(r.errors.is_empty(), "{}: {:?}", m.name, r.errors);
    }
    // A code on a score no spec chose refuses nothing: C9 alone holds 2.
    let mut one = stats_desc::spec(
        OperationId(1),
        &d,
        DescribeParams {
            columns: vec![ColumnRequest {
                alias: a(SUM),
                treat_as: TreatAs::Numeric,
            }],
            missing_codes: vec![2.0],
        },
    );
    one.preparation = Some(preparation());
    let r = report_of(stats_desc::validate(&d, &one));
    assert!(r.errors.is_empty(), "{:?}", r.errors);
}

/// The refusal is for a run too, whatever reached it.
#[test]
fn a_run_with_a_code_equal_to_a_scores_value_is_refused_in_each_method() {
    let d = prepared();
    let code = [99.0];
    let want = |prefix: &str| format!("{prefix}code_equals_score");
    assert_eq!(
        stats_desc::describe(&d, &describe_params(&code))
            .unwrap_err()
            .code
            .wire(),
        want("stats.desc.")
    );
    assert_eq!(
        stats_ttest::compute(&d, &ttest_params(&code))
            .unwrap_err()
            .code
            .wire(),
        want("stats.ttest.")
    );
    assert_eq!(
        stats_onesample::one_sample_compute(&d, &one_sample_params(&code))
            .unwrap_err()
            .code
            .wire(),
        want("stats.onesample.")
    );
    assert_eq!(
        stats_onesample::paired_compute(&d, &paired_params(&code))
            .unwrap_err()
            .code
            .wire(),
        want("stats.onesample.")
    );
    assert_eq!(
        stats_anova::compute(&d, &anova_params(&code))
            .unwrap_err()
            .code
            .wire(),
        want("stats.anova.")
    );
    assert_eq!(
        stats_corr::compute(&d, &corr_params(&code))
            .unwrap_err()
            .code
            .wire(),
        want("stats.corr.")
    );
    assert_eq!(
        stats_alpha::compute(&d, &alpha_params(&code))
            .unwrap_err()
            .code
            .wire(),
        want("stats.alpha.")
    );
    assert_eq!(
        stats_ranks::compute(&d, &ranks_params(&code))
            .unwrap_err()
            .code
            .wire(),
        want("stats.ranks.")
    );
}

/// The paired test takes two columns, and either may be the score with the 99.
#[test]
fn a_paired_specs_second_column_is_checked_for_a_code_equal_to_a_score() {
    let d = prepared();
    let params = PairedParams {
        first: a(MEAN),
        second: a(SUM),
        missing_codes: vec![99.0],
    };
    let mut spec = stats_onesample::paired_spec(OperationId(1), &d, params.clone());
    spec.preparation = Some(preparation());
    let r = report_of(stats_onesample::paired_validate(&d, &spec));
    assert_eq!(
        r.errors,
        [(
            "stats.onesample.code_equals_score".to_string(),
            Some(i64::from(SUM)),
            Some(1)
        )]
    );
    let refused = stats_onesample::paired_compute(&d, &params).unwrap_err();
    assert_eq!(refused.code.wire(), "stats.onesample.code_equals_score");
}

/// The same 99 as a file's column is a code like any other, in each method.
#[test]
fn a_ticked_code_on_a_files_own_column_is_applied_as_before() {
    let d = as_a_file();
    for m in methods() {
        let r = (m.validate)(&d, None, &d, &[99.0]);
        assert!(r.errors.is_empty(), "{}: {:?}", m.name, r.errors);
    }
}

/* ---- the suspect-code rule --------------------------------------------------- */

#[test]
fn the_suspect_code_rule_is_not_applied_to_a_score_in_each_methods_validate() {
    let (score, file) = (prepared(), as_a_file());
    for m in methods() {
        let want = format!("{}suspect_missing_code", m.prefix);
        let on_score = (m.validate)(&score, Some(preparation()), &score, &[]);
        assert_eq!(on_score.status, ValidationStatus::Valid, "{}", m.name);
        assert!(
            on_score.warnings.is_empty(),
            "{}: {:?}",
            m.name,
            on_score.warnings
        );
        // The control: the same 99 as a file's column reaches the rule.
        let on_file = (m.validate)(&file, None, &file, &[]);
        assert_eq!(
            on_file.status,
            ValidationStatus::ValidWithWarnings,
            "{}",
            m.name
        );
        assert_eq!(on_file.warnings, [want], "{}", m.name);
    }
}

#[test]
fn the_suspect_code_rule_is_not_applied_to_a_score_in_the_profile() {
    let suspects = |d: &Dataset, alias: u32| {
        stats_desc::profile(d)
            .columns
            .iter()
            .find(|c| c.alias == a(alias))
            .expect("the column")
            .suspect_missing_codes
            .len()
    };
    assert_eq!(suspects(&prepared(), SUM), 0);
    assert_eq!(suspects(&as_a_file(), SUM), 1, "the control");
}

/// A candidate code in a file's column of a prepared table still warns: the
/// exemption is for scores, not for prepared tables.
#[test]
fn a_prepared_tables_file_columns_keep_the_suspect_code_rule() {
    let rows: Vec<Vec<String>> = ROWS
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut row: Vec<String> = r.iter().map(|c| c.to_string()).collect();
            if i == 3 {
                row[0] = "-999".to_string();
            }
            row
        })
        .collect();
    let s = table(
        HEADERS.iter().map(|h| h.to_string()).collect(),
        rows,
        b"a source with a code in its id column",
    );
    let d = prepared_with(&s, &recipe()).dataset;
    let params = DescribeParams {
        columns: vec![ColumnRequest {
            alias: a(1),
            treat_as: TreatAs::Numeric,
        }],
        missing_codes: Vec::new(),
    };
    let mut spec = stats_desc::spec(OperationId(1), &d, params);
    spec.preparation = Some(preparation_of(&s, &recipe()));
    let r = report_of(stats_desc::validate(&d, &spec));
    assert_eq!(r.warnings, ["stats.desc.suspect_missing_code"]);
    let profile = stats_desc::profile(&d);
    assert_eq!(profile.columns[0].suspect_missing_codes.len(), 1);
}

/* ---- crosstabs (Phase 8X-1, decision 577) ---------------------------------- */
//
// Crosstabs refuses a score as either column, so it is not among `methods()`,
// whose tests run each method on a score. Its preparation checks run here on
// two of the file's columns of the prepared table, the group and the level.

fn xtab_params(rows: u32, columns: u32) -> CrosstabsParams {
    CrosstabsParams {
        rows: a(rows),
        columns: a(columns),
        missing_codes: Vec::new(),
    }
}

/// Crosstabs' validate on `on` of a spec made on `made_on` with this
/// preparation, over the group and the level.
fn xtab_validate(made_on: &Dataset, prep: Option<Preparation>, on: &Dataset) -> Report {
    let mut spec = stats_xtab::spec(OperationId(1), made_on, xtab_params(GRP, LVL));
    spec.preparation = prep;
    report_of(stats_xtab::validate(on, &spec))
}

#[test]
fn a_score_is_refused_as_either_column_of_crosstabs() {
    let d = prepared();
    for (rows, columns) in [(SUM, GRP), (GRP, SUM)] {
        let params = xtab_params(rows, columns);
        let mut spec = stats_xtab::spec(OperationId(1), &d, params.clone());
        spec.preparation = Some(preparation());
        let r = report_of(stats_xtab::validate(&d, &spec));
        assert_eq!(
            r.errors,
            [(
                "stats.xtab.score_as_column".to_string(),
                Some(i64::from(SUM)),
                None
            )],
            "rows C{rows}, columns C{columns}"
        );
        let refused = stats_xtab::compute(&d, &params).unwrap_err();
        assert_eq!(refused.code.wire(), "stats.xtab.score_as_column");
    }
    // Both columns scores: each named, the rows' first.
    let mut spec = stats_xtab::spec(OperationId(1), &d, xtab_params(SUM, MEAN));
    spec.preparation = Some(preparation());
    let r = report_of(stats_xtab::validate(&d, &spec));
    assert_eq!(
        r.errors,
        [
            ("stats.xtab.score_as_column".to_string(), Some(i64::from(SUM)), None),
            ("stats.xtab.score_as_column".to_string(), Some(i64::from(MEAN)), None),
        ]
    );
}

/// The same numbers as a file's column are a column like any other: it is
/// the column's origin that refuses, not its values.
#[test]
fn the_same_values_as_a_files_column_may_be_a_crosstabs_column() {
    let d = as_a_file();
    let r = report_of(stats_xtab::validate(
        &d,
        &stats_xtab::spec(OperationId(1), &d, xtab_params(SUM, GRP)),
    ));
    assert!(
        !words(&r.errors).contains(&"stats.xtab.score_as_column"),
        "{:?}",
        r.errors
    );
    assert!(stats_xtab::compute(&d, &xtab_params(SUM, GRP)).is_ok());
}

#[test]
fn a_forged_preparation_is_refused_in_crosstabs_and_nothing_else_is_said() {
    let d = prepared();
    let want = "stats.xtab.preparation_mismatch";
    let good = xtab_validate(&d, Some(preparation()), &d);
    assert!(good.errors.is_empty(), "{:?}", good.errors);
    let other_hash = ContentHash::of_bytes(b"something else");
    let forgeries: [(&str, Box<dyn Fn(&mut Preparation)>); 5] = [
        (
            "source hash",
            Box::new({
                let h = other_hash.clone();
                move |p| p.source_hash = h.clone()
            }),
        ),
        (
            "recipe hash",
            Box::new({
                let h = other_hash.clone();
                move |p| p.recipe_hash = h.clone()
            }),
        ),
        ("source columns", Box::new(|p| p.source_columns += 1)),
        (
            "capability version",
            Box::new(|p| p.capability_version += 1),
        ),
        ("engine version", Box::new(|p| p.engine_version += 1)),
    ];
    for (what, forge) in &forgeries {
        let mut forged = preparation();
        forge(&mut forged);
        let r = xtab_validate(&d, Some(forged), &d);
        assert_eq!(r.status, ValidationStatus::Invalid, "{what}");
        assert_eq!(words(&r.errors), [want], "{what}");
    }
    let absent = xtab_validate(&d, None, &d);
    assert_eq!(words(&absent.errors), [want], "absent");
}

#[test]
fn two_recipes_of_one_width_are_refused_for_each_other_in_crosstabs() {
    let s = source();
    let (r1, r2) = (recipe(), other_recipe());
    let (d1, d2) = (
        prepared_with(&s, &r1).dataset,
        prepared_with(&s, &r2).dataset,
    );
    let carried = xtab_validate(&d1, Some(preparation_of(&s, &r1)), &d2);
    let got = words(&carried.errors);
    assert!(got.contains(&"stats.xtab.input_mismatch"), "{got:?}");
    assert!(got.contains(&"stats.xtab.preparation_mismatch"), "{got:?}");
    let forged = xtab_validate(&d2, Some(preparation_of(&s, &r1)), &d2);
    assert_eq!(words(&forged.errors), ["stats.xtab.preparation_mismatch"]);
}

#[test]
fn a_crosstabs_spec_made_on_the_original_is_refused_on_the_prepared_table_and_the_reverse() {
    let (s, d) = (source(), prepared());
    let on_prepared = xtab_validate(&s, None, &d);
    let got = words(&on_prepared.errors);
    assert!(got.contains(&"stats.xtab.input_mismatch"), "{got:?}");
    assert!(got.contains(&"stats.xtab.preparation_mismatch"), "{got:?}");
    let on_original = xtab_validate(&d, Some(preparation()), &s);
    let got = words(&on_original.errors);
    assert_eq!(on_original.status, ValidationStatus::Invalid);
    assert!(got.contains(&"stats.xtab.input_mismatch"), "{got:?}");
    assert!(got.contains(&"stats.xtab.preparation_mismatch"), "{got:?}");
}
