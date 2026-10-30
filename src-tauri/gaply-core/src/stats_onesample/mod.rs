//! **The one-sample and paired-samples t-tests as analyses of the user's own
//! data (Research OS, Phase 8D-1): `stats.analyze.one_sample_t` and
//! `stats.analyze.paired_t`.**
//!
//! Not reproductions of a reported result: Level 1's
//! `stats.verify.paired_t` is that. This module computes each test on the
//! user's columns, with no claim to compare against.
//!
//! * The one-sample t-test compares the mean of one numeric column with a
//!   test value the user gives. The test value is never chosen for the user:
//!   until the spec holds one, validation needs input (decision 255).
//! * The paired t-test compares a first and a second numeric column measured
//!   on the same rows, through the differences first minus second. It IS the
//!   one-sample t-test of those differences against 0, and is computed so:
//!   `stats_verify::t_test_paired` calls the one-sample arithmetic on them.
//! * For each: `*_validate` checks a spec against the dataset, `*_run`
//!   computes after one check point, `*_manifest` records the run, and
//!   [`export`] writes the result as CSV and the manifest as JSON.
//!
//! Pure: no file, network, process, clock or model; the caller passes the
//! elapsed time and the platform (`tests/contracts_are_pure.rs`).
//!
//! **One implementation of each formula (decision 254).** t, df and the
//! standard error come from `stats_verify::t_test_one_sample` and
//! `t_test_paired`; p from `stats_ttest::two_sided_p` and the 95% interval
//! from `stats_ttest::interval_95`, the t family's one p and one interval,
//! both through `stats_dist` (`stats_l0::pvalue`'s values at df ≤ 1e4, bit
//! for bit; Hill's p and the Cornish–Fisher quantile above, to df 1e7;
//! Research OS, Phase 8E, decision 275);
//! every mean and variance from `stats_verify::mean` and `sample_variance`.
//! Nothing here computes a variance, a quantile or a p of its own.
//!
//! **The data (decision 255).** Each chosen column must hold numbers only. A
//! row is left out once, by the first reason that applies, in this order:
//! `missing_value`, then `missing_code` (one sample); `missing_first`,
//! `missing_second`, then `missing_code` (paired). The user's ticked
//! codes apply to the one-sample test's column and to both of the paired
//! test's columns. Before the arithmetic the one-sample test's values are
//! sorted with `total_cmp`, and the paired test's pairs by their difference,
//! then the first, then the second, so permuting the rows changes no bit.
//!
//! **Edge cases are closed outcomes, never NaN or infinity.** Fewer than two
//! values or pairs is refused (`too_few_values`, `too_few_pairs`); values, or
//! differences, that are all equal have a standard error of exactly 0 and are
//! `values_constant` or `differences_constant`; a standard error that is not
//! 0 but below 10 · ε · a scale, spread lost in rounding, is
//! `values_essentially_constant` (scale |mean|, as R's "data are essentially
//! constant") or `differences_essentially_constant` (scale the largest of
//! |mean of the differences|, the mean of |first| and the mean of |second|:
//! R uses only the first, and the other two are Gaply's own, because the
//! rounding in first minus second grows with the size of the values;
//! decision 259, `stats_ttest::essentially_constant`); above
//! `stats_ttest::QUANTILE_MAX_DF` degrees of freedom (1e7 since 8E; 1e4
//! before it, decision 242.7) the run is `quantile_out_of_range`; a sum,
//! square or root a double
//! cannot hold is `overflow`, a sum of squares that falls below the smallest
//! normal double although the values differ is `underflow`, and any other
//! refusal of the arithmetic is `arithmetic_refused`.
//!
//! **Not assessed, and said so in every result:** normality of the values or
//! of the differences, and Cohen's d has no interval (decision 63). The
//! wording is neutral: no verdict.
//!
//! **Privacy.** No type here holds text: the results are numbers, closed
//! words and counts. The values themselves never leave [`Values`] and
//! [`Pairs`], which derive nothing.

pub mod export;

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::ops::ControlFlow;

use serde::{Deserialize, Serialize};

use crate::contracts::codes::closed;
use crate::contracts::{
    CapabilityId, ContentHash, Diagnostic, Exclusion, OperationId, OperationSpec, ParamKey,
    ParamValue, Platform, Reproducibility, ResourceEstimate, RunManifest, SchemaVersion,
    ValidationReport, WorkspaceId,
};
use crate::stats_desc::{input_ref, Alias};
use crate::stats_l1::binding::suspect_codes_in;
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_ttest::{
    column, essentially_constant, interval_95, non_finite_cells, two_sided_p,
};
use crate::stats_verify::{mean, sample_variance, t_test_one_sample, t_test_paired, RangeFault};

/// The one-sample t-test's id in the registry.
pub const ONE_SAMPLE_ID: &str = "stats.analyze.one_sample_t";
/// The paired t-test's id in the registry.
pub const PAIRED_ID: &str = "stats.analyze.paired_t";
/// Raised when what a one-sample spec means changes.
pub const ONE_SAMPLE_VERSION: u32 = 1;
/// Raised when what a paired spec means changes.
pub const PAIRED_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// The interval's coverage. Fixed in version 1.
pub const CONFIDENCE_LEVEL: f64 = 0.95;

closed! {
    /// Every finding, outcome, exclusion and limitation of the two tests.
    pub enum OneSampleCode in "stats.onesample." {
        /// The spec names another capability.
        UnknownCapability => "unknown_capability",
        /// The spec was written for another version of this capability.
        CapabilityVersionMismatch => "capability_version_mismatch",
        /// The spec does not name exactly one input.
        InputCount => "input_count",
        /// The spec's input is not this dataset.
        InputMismatch => "input_mismatch",
        /// No column has that alias. Param `column`.
        UnknownColumn => "unknown_column",
        /// The paired test's first and second are one column.
        SameColumn => "same_column",
        /// A chosen column holds text. Params `column`, `count`.
        ColumnHoldsText => "column_holds_text",
        /// A chosen column holds NaN or an infinity. Params `column`, `count`.
        ColumnNonFinite => "column_non_finite",
        /// A missing code is NaN or an infinity. Param `index`.
        MissingCodeNotFinite => "missing_code_not_finite",
        /// Needs input: the user has not given the one-sample test's value.
        TestValueNotChosen => "test_value_not_chosen",
        /// The test value is NaN or an infinity.
        TestValueNotFinite => "test_value_not_finite",
        /// A candidate missing code the rule found in a chosen column and the
        /// user did not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Refused: fewer than two values after the exclusions. Param `count`.
        TooFewValues => "too_few_values",
        /// Refused: fewer than two pairs after the exclusions. Param `count`.
        TooFewPairs => "too_few_pairs",
        /// Not computable: the values are all equal, so the standard error is
        /// exactly 0.
        ValuesConstant => "values_constant",
        /// Not computable: the differences are all equal, so the standard
        /// error is exactly 0.
        DifferencesConstant => "differences_constant",
        /// Not computable: a sum, square, quotient or root a double cannot
        /// hold (decision 70).
        Overflow => "overflow",
        /// Not computable: a sum of squares underflowed though the values
        /// differ (decision 70).
        Underflow => "underflow",
        /// Not computable: the arithmetic refused for a reason it did not
        /// name.
        ArithmeticRefused => "arithmetic_refused",
        /// Not computable: the t distribution is not measured at this many
        /// degrees of freedom (above `stats_ttest::QUANTILE_MAX_DF`, 1e7).
        QuantileOutOfRange => "quantile_out_of_range",
        /// Excluded: the row's value is empty or a missing-value token.
        MissingValue => "missing_value",
        /// Excluded: the row's first value is empty or a missing-value token.
        MissingFirst => "missing_first",
        /// Excluded: the row's second value is empty or a missing-value token.
        MissingSecond => "missing_second",
        /// Excluded: a value of the row is one the user listed.
        MissingCode => "missing_code",
        /// Limitation: the values' normality is not assessed.
        ValuesNormalityNotAssessed => "values_normality_not_assessed",
        /// Limitation: the differences' normality is not assessed.
        DifferencesNormalityNotAssessed => "differences_normality_not_assessed",
        /// Limitation: Cohen's d has no confidence interval (decision 63).
        CohensDHasNoInterval => "cohens_d_has_no_interval",
        /// Not computable: the standard error is not 0 but below 10 · ε ·
        /// |mean|, so the values' spread is lost in rounding and t means
        /// nothing; R stops on the same rule (decision 259).
        ValuesEssentiallyConstant => "values_essentially_constant",
        /// Not computable: the standard error is not 0 but below 10 · ε ·
        /// the largest of |mean of the differences|, the mean of |first| and
        /// the mean of |second|, so the differences' spread is lost in
        /// rounding; R uses only the first of the three (decision 259).
        DifferencesEssentiallyConstant => "differences_essentially_constant",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// The one-sample t-test's parameters. `test_value` must be given before the
/// spec can run, and is never defaulted. `missing_codes` apply to `outcome`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OneSampleParams {
    pub outcome: Alias,
    pub test_value: Option<f64>,
    pub missing_codes: Vec<f64>,
}

/// The paired t-test's parameters. The difference is `first` minus
/// `second`; nothing here chooses which is first. `missing_codes` apply to
/// both columns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairedParams {
    pub first: Alias,
    pub second: Alias,
    pub missing_codes: Vec<f64>,
}

/* ------------------------------------------------------------------------ *
 *  The results
 * ------------------------------------------------------------------------ */

/// The one-sample t-test's result. Every number is finite.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OneSampleResult {
    /// The values used.
    pub n: u64,
    pub mean: f64,
    pub sd: f64,
    pub test_value: f64,
    pub t: f64,
    pub df: f64,
    /// Two-sided.
    pub p: f64,
    /// The mean minus the test value.
    pub mean_difference: f64,
    /// The standard error t divides by: sd / sqrt(n).
    pub standard_error: f64,
    pub confidence_level: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
    /// The mean difference over the sd.
    pub cohens_d: f64,
    /// The rows left out, by reason; reasons with no row are not listed.
    pub exclusions: Vec<Exclusion<OneSampleCode>>,
    pub limitations: Vec<OneSampleCode>,
}

/// One column of the paired test, over the pairs used.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnStats {
    pub mean: f64,
    pub sd: f64,
}

/// The paired t-test's result. Every number is finite.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedResult {
    /// The pairs used.
    pub n_pairs: u64,
    pub first: ColumnStats,
    pub second: ColumnStats,
    pub t: f64,
    pub df: f64,
    /// Two-sided.
    pub p: f64,
    /// The mean of the differences first minus second.
    pub mean_difference: f64,
    /// The sd of the differences.
    pub sd_differences: f64,
    /// The standard error t divides by: sd of the differences / sqrt(n).
    pub standard_error: f64,
    pub confidence_level: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
    /// Cohen's d_z: the mean difference over the sd of the differences.
    pub cohens_dz: f64,
    /// The rows left out, by reason; reasons with no row are not listed.
    pub exclusions: Vec<Exclusion<OneSampleCode>>,
    pub limitations: Vec<OneSampleCode>,
}

impl OneSampleResult {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers and closed words serialises")
    }
}

impl PairedResult {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers and closed words serialises")
    }
}

/// What every one-sample result says it did not assess.
const ONE_SAMPLE_LIMITATIONS: [OneSampleCode; 2] = [
    OneSampleCode::ValuesNormalityNotAssessed,
    OneSampleCode::CohensDHasNoInterval,
];
/// What every paired result says it did not assess.
const PAIRED_LIMITATIONS: [OneSampleCode; 2] = [
    OneSampleCode::DifferencesNormalityNotAssessed,
    OneSampleCode::CohensDHasNoInterval,
];

/* ------------------------------------------------------------------------ *
 *  The one reading pass of each test
 * ------------------------------------------------------------------------ */

/// The one-sample test's values, ascending, and the rows left out. Never
/// leaves this module, and derives nothing that could print or serialise it.
struct Values {
    values: Vec<f64>,
    /// `missing_value`, `missing_code`.
    excluded: [u64; 2],
}

const ONE_SAMPLE_EXCLUSIONS: [OneSampleCode; 2] =
    [OneSampleCode::MissingValue, OneSampleCode::MissingCode];

fn values_of(cells: &[Cell], codes: &[f64]) -> Values {
    let mut v = Values {
        values: Vec::new(),
        excluded: [0; 2],
    };
    for cell in cells {
        match cell {
            Cell::Missing => v.excluded[0] += 1,
            Cell::Number(x) if codes.contains(x) => v.excluded[1] += 1,
            Cell::Number(x) => v.values.push(*x),
            // Refused by validation before any reading is used.
            Cell::Text(_) => v.excluded[0] += 1,
        }
    }
    v.values.sort_by(f64::total_cmp);
    v
}

/// The paired test's pairs, sorted by their difference, then the first, then
/// the second, and the rows left out. Never leaves this module, and derives
/// nothing that could print or serialise it.
struct Pairs {
    first: Vec<f64>,
    second: Vec<f64>,
    /// `missing_first`, `missing_second`, `missing_code`.
    excluded: [u64; 3],
}

const PAIRED_EXCLUSIONS: [OneSampleCode; 3] = [
    OneSampleCode::MissingFirst,
    OneSampleCode::MissingSecond,
    OneSampleCode::MissingCode,
];

fn pairs_of(first: &[Cell], second: &[Cell], codes: &[f64]) -> Pairs {
    let mut excluded = [0; 3];
    let mut kept: Vec<(f64, f64)> = Vec::new();
    for (a, b) in first.iter().zip(second) {
        // Text is refused by validation before any reading is used; it is
        // counted as missing here, as a missing cell.
        let (Cell::Number(x), Cell::Number(y)) = (a, b) else {
            if !matches!(a, Cell::Number(_)) {
                excluded[0] += 1;
            } else {
                excluded[1] += 1;
            }
            continue;
        };
        if codes.contains(x) || codes.contains(y) {
            excluded[2] += 1;
            continue;
        }
        kept.push((*x, *y));
    }
    kept.sort_by(|p, q| {
        (p.0 - p.1)
            .total_cmp(&(q.0 - q.1))
            .then(p.0.total_cmp(&q.0))
            .then(p.1.total_cmp(&q.1))
    });
    Pairs {
        first: kept.iter().map(|p| p.0).collect(),
        second: kept.iter().map(|p| p.1).collect(),
        excluded,
    }
}

/* ------------------------------------------------------------------------ *
 *  Specs and the checks they share
 * ------------------------------------------------------------------------ */

/// The one-sample test's id, parsed.
pub fn one_sample_capability_id() -> CapabilityId {
    CapabilityId::parse(ONE_SAMPLE_ID).expect("the capability id is well formed")
}

/// The paired test's id, parsed.
pub fn paired_capability_id() -> CapabilityId {
    CapabilityId::parse(PAIRED_ID).expect("the capability id is well formed")
}

fn spec_of<P>(
    operation_id: OperationId,
    dataset: &Dataset,
    capability_id: CapabilityId,
    capability_version: u32,
    parameters: P,
) -> OperationSpec<P> {
    OperationSpec {
        schema_version: SchemaVersion,
        operation_id,
        workspace_id: WorkspaceId::Statistics,
        capability_id,
        capability_version,
        inputs: vec![input_ref(dataset)],
        parameters,
        random_seed: None,
        confirmation_refs: Vec::new(),
    }
}

/// A one-sample spec on this dataset.
pub fn one_sample_spec(
    operation_id: OperationId,
    dataset: &Dataset,
    parameters: OneSampleParams,
) -> OperationSpec<OneSampleParams> {
    spec_of(
        operation_id,
        dataset,
        one_sample_capability_id(),
        ONE_SAMPLE_VERSION,
        parameters,
    )
}

/// A paired spec on this dataset.
pub fn paired_spec(
    operation_id: OperationId,
    dataset: &Dataset,
    parameters: PairedParams,
) -> OperationSpec<PairedParams> {
    spec_of(
        operation_id,
        dataset,
        paired_capability_id(),
        PAIRED_VERSION,
        parameters,
    )
}

fn column_param(alias: Alias) -> ParamValue<OneSampleCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

/// The spec's capability, version and workspace, against `id` and
/// `version`: an unsupported finding for each that is not this test's.
fn unsupported_of<P>(
    spec: &OperationSpec<P>,
    id: &str,
    version: u32,
) -> Vec<Diagnostic<OneSampleCode>> {
    let mut out = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != id {
        out.push(Diagnostic::error(OneSampleCode::UnknownCapability));
    }
    if spec.capability_version != version {
        out.push(
            Diagnostic::error(OneSampleCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(version))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    out
}

/// The spec's one input, against this dataset.
fn input_errors<P>(spec: &OperationSpec<P>, dataset: &Dataset) -> Vec<Diagnostic<OneSampleCode>> {
    match spec.inputs.as_slice() {
        [only] if *only == input_ref(dataset) => Vec::new(),
        [_] => vec![Diagnostic::error(OneSampleCode::InputMismatch)],
        inputs => {
            vec![Diagnostic::error(OneSampleCode::InputCount)
                .int(ParamKey::Count, inputs.len() as i64)]
        }
    }
}

fn missing_code_errors(codes: &[f64]) -> Vec<Diagnostic<OneSampleCode>> {
    codes
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.is_finite())
        .map(|(i, _)| {
            Diagnostic::error(OneSampleCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64)
        })
        .collect()
}

/// A chosen column's cells, or `unknown_column`; and `column_non_finite` or
/// `column_holds_text` when it holds anything but numbers and missing cells.
fn numeric_column<'d>(
    dataset: &'d Dataset,
    alias: Alias,
    errors: &mut Vec<Diagnostic<OneSampleCode>>,
) -> Option<&'d [Cell]> {
    let Some((summary, cells)) = column(dataset, alias) else {
        errors.push(
            Diagnostic::error(OneSampleCode::UnknownColumn)
                .with(ParamKey::Column, column_param(alias)),
        );
        return None;
    };
    let non_finite = non_finite_cells(summary);
    let texts = cells.iter().filter(|c| matches!(c, Cell::Text(_))).count() as i64;
    if non_finite > 0 {
        errors.push(
            Diagnostic::error(OneSampleCode::ColumnNonFinite)
                .with(ParamKey::Column, column_param(alias))
                .int(ParamKey::Count, non_finite as i64),
        );
    } else if texts > 0 {
        errors.push(
            Diagnostic::error(OneSampleCode::ColumnHoldsText)
                .with(ParamKey::Column, column_param(alias))
                .int(ParamKey::Count, texts),
        );
    }
    Some(cells)
}

/// The candidate missing codes in a chosen column that the user did not list.
fn suspect_warnings(
    dataset: &Dataset,
    alias: Alias,
    codes: &[f64],
) -> Vec<Diagnostic<OneSampleCode>> {
    let Some((_, cells)) = column(dataset, alias) else {
        return Vec::new();
    };
    suspect_codes_in(alias.0, cells)
        .into_iter()
        .filter(|s| !codes.contains(&f64::from(s.code)))
        .map(|s| {
            Diagnostic::warning(OneSampleCode::SuspectMissingCode)
                .with(ParamKey::Column, column_param(alias))
                .int(ParamKey::Value, i64::from(s.code))
                .int(ParamKey::Count, s.cells as i64)
        })
        .collect()
}

/// What the parameters leave wrong or open on this dataset.
struct Checked<T> {
    errors: Vec<Diagnostic<OneSampleCode>>,
    questions: Vec<Diagnostic<OneSampleCode>>,
    /// The reading, when nothing is wrong.
    ready: Option<T>,
}

/// The one-sample checks, shared by validate and run: errors in a fixed order
/// (missing codes, the test value, the column, then the count), and the open
/// question (the test value).
fn check_one_sample(dataset: &Dataset, params: &OneSampleParams) -> Checked<Values> {
    let mut errors = missing_code_errors(&params.missing_codes);
    let mut questions = Vec::new();
    match params.test_value {
        None => questions.push(Diagnostic::note(OneSampleCode::TestValueNotChosen)),
        Some(m) if !m.is_finite() => {
            errors.push(Diagnostic::error(OneSampleCode::TestValueNotFinite))
        }
        Some(_) => {}
    }
    let cells = numeric_column(dataset, params.outcome, &mut errors);
    let mut ready = None;
    if errors.is_empty() {
        if let Some(cells) = cells {
            let v = values_of(cells, &params.missing_codes);
            if v.values.len() < 2 {
                errors.push(
                    Diagnostic::error(OneSampleCode::TooFewValues)
                        .int(ParamKey::Count, v.values.len() as i64),
                );
            } else {
                ready = Some(v);
            }
        }
    }
    Checked {
        errors,
        questions,
        ready,
    }
}

/// The paired checks, shared by validate and run: errors in a fixed order
/// (missing codes, the two columns, the same column, then the count).
fn check_paired(dataset: &Dataset, params: &PairedParams) -> Checked<Pairs> {
    let mut errors = missing_code_errors(&params.missing_codes);
    let first = numeric_column(dataset, params.first, &mut errors);
    let second = numeric_column(dataset, params.second, &mut errors);
    if first.is_some() && second.is_some() && params.first == params.second {
        errors.push(Diagnostic::error(OneSampleCode::SameColumn));
    }
    let mut ready = None;
    if errors.is_empty() {
        if let (Some(a), Some(b)) = (first, second) {
            let p = pairs_of(a, b, &params.missing_codes);
            if p.first.len() < 2 {
                errors.push(
                    Diagnostic::error(OneSampleCode::TooFewPairs)
                        .int(ParamKey::Count, p.first.len() as i64),
                );
            } else {
                ready = Some(p);
            }
        }
    }
    Checked {
        errors,
        questions: Vec::new(),
        ready,
    }
}

fn estimate(dataset: &Dataset, columns: u64) -> ResourceEstimate {
    let rows = dataset.rows() as u64;
    ResourceEstimate {
        rows,
        columns,
        cells: rows.saturating_mul(columns),
    }
}

/// Checks a one-sample spec against the dataset. Unsupported when it names
/// another capability, version or workspace; invalid on a wrong input or a
/// parameter error; needs input while the test value is not given; a warning
/// for a candidate missing code in the column that the user did not list.
pub fn one_sample_validate(
    dataset: &Dataset,
    spec: &OperationSpec<OneSampleParams>,
) -> ValidationReport<OneSampleCode> {
    let params = &spec.parameters;
    let mut errors = input_errors(spec, dataset);
    let checked = check_one_sample(dataset, params);
    errors.extend(checked.errors);
    ValidationReport::new(
        unsupported_of(spec, ONE_SAMPLE_ID, ONE_SAMPLE_VERSION),
        errors,
        checked.questions,
        suspect_warnings(dataset, params.outcome, &params.missing_codes),
        estimate(dataset, 1),
    )
}

/// Checks a paired spec against the dataset, as [`one_sample_validate`]
/// does; the warnings cover both columns, the first's first.
pub fn paired_validate(
    dataset: &Dataset,
    spec: &OperationSpec<PairedParams>,
) -> ValidationReport<OneSampleCode> {
    let params = &spec.parameters;
    let mut errors = input_errors(spec, dataset);
    let checked = check_paired(dataset, params);
    errors.extend(checked.errors);
    let mut warnings = suspect_warnings(dataset, params.first, &params.missing_codes);
    if params.second != params.first {
        warnings.extend(suspect_warnings(
            dataset,
            params.second,
            &params.missing_codes,
        ));
    }
    ValidationReport::new(
        unsupported_of(spec, PAIRED_ID, PAIRED_VERSION),
        errors,
        checked.questions,
        warnings,
        estimate(dataset, 2),
    )
}

/* ------------------------------------------------------------------------ *
 *  run
 * ------------------------------------------------------------------------ */

/// Why a run returned no result.
#[derive(Debug, Clone, PartialEq)]
pub enum Halt<B> {
    /// The parameters are wrong or open for this dataset, or the result is
    /// not computable.
    Refused(Diagnostic<OneSampleCode>),
    /// The caller's check point stopped the run before the arithmetic.
    Stopped { reason: B },
}

/// The first error, then the first question, of a check; else its reading.
fn ready_or_halt<T, B>(checked: Checked<T>) -> Result<T, Halt<B>> {
    if let Some(e) = checked.errors.into_iter().next() {
        return Err(Halt::Refused(e));
    }
    if let Some(q) = checked.questions.into_iter().next() {
        return Err(Halt::Refused(q));
    }
    Ok(checked
        .ready
        .expect("no error and no question means the reading is ready"))
}

/// The one-sample t-test, with the same check point as `stats_ttest::run`:
/// `checkpoint(0)` is called once, after the parameters are checked and
/// before the arithmetic, and a `Break` stops the run there.
pub fn one_sample_run<B>(
    dataset: &Dataset,
    params: &OneSampleParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<OneSampleResult, Halt<B>> {
    let v = ready_or_halt(check_one_sample(dataset, params))?;
    let test_value = params
        .test_value
        .expect("no question means the test value is given");
    if let ControlFlow::Break(reason) = checkpoint(0) {
        return Err(Halt::Stopped { reason });
    }
    let n = one_sample_numbers(&v.values, test_value)
        .map_err(|c| Halt::Refused(Diagnostic::error(c)))?;
    let exclusions = exclusions_of(&ONE_SAMPLE_EXCLUSIONS, &v.excluded, |_| {
        Some(params.outcome)
    });
    Ok(OneSampleResult {
        n: v.values.len() as u64,
        mean: n.mean,
        sd: n.sd,
        test_value,
        t: n.t,
        df: n.df,
        p: n.p,
        mean_difference: n.mean_difference,
        standard_error: n.se,
        confidence_level: CONFIDENCE_LEVEL,
        ci_lower: n.ci[0],
        ci_upper: n.ci[1],
        cohens_d: n.d,
        exclusions,
        limitations: ONE_SAMPLE_LIMITATIONS.to_vec(),
    })
}

/// [`one_sample_run`] with a check point that never stops.
pub fn one_sample_compute(
    dataset: &Dataset,
    params: &OneSampleParams,
) -> Result<OneSampleResult, Diagnostic<OneSampleCode>> {
    match one_sample_run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The paired t-test, with the same check point as [`one_sample_run`].
pub fn paired_run<B>(
    dataset: &Dataset,
    params: &PairedParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<PairedResult, Halt<B>> {
    let p = ready_or_halt(check_paired(dataset, params))?;
    if let ControlFlow::Break(reason) = checkpoint(0) {
        return Err(Halt::Stopped { reason });
    }
    let n = paired_numbers(&p.first, &p.second).map_err(|c| Halt::Refused(Diagnostic::error(c)))?;
    let exclusions = exclusions_of(&PAIRED_EXCLUSIONS, &p.excluded, |code| match code {
        OneSampleCode::MissingFirst => Some(params.first),
        OneSampleCode::MissingSecond => Some(params.second),
        // A row left out for a code in either column names neither.
        _ => None,
    });
    Ok(PairedResult {
        n_pairs: p.first.len() as u64,
        first: n.first,
        second: n.second,
        t: n.t,
        df: n.df,
        p: n.p,
        mean_difference: n.mean_difference,
        sd_differences: n.sd_differences,
        standard_error: n.se,
        confidence_level: CONFIDENCE_LEVEL,
        ci_lower: n.ci[0],
        ci_upper: n.ci[1],
        cohens_dz: n.dz,
        exclusions,
        limitations: PAIRED_LIMITATIONS.to_vec(),
    })
}

/// [`paired_run`] with a check point that never stops.
pub fn paired_compute(
    dataset: &Dataset,
    params: &PairedParams,
) -> Result<PairedResult, Diagnostic<OneSampleCode>> {
    match paired_run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The exclusions with at least one row, each naming its column when it has
/// one.
fn exclusions_of(
    codes: &[OneSampleCode],
    counts: &[u64],
    column_of: impl Fn(OneSampleCode) -> Option<Alias>,
) -> Vec<Exclusion<OneSampleCode>> {
    codes
        .iter()
        .zip(counts)
        .filter(|(_, n)| **n > 0)
        .map(|(code, count)| Exclusion {
            code: *code,
            count: *count,
            params: match column_of(*code) {
                Some(alias) => BTreeMap::from([(ParamKey::Column, column_param(alias))]),
                None => BTreeMap::new(),
            },
        })
        .collect()
}

/// `v`, or `overflow` when it is infinite or NaN: every number here is a
/// sum, product, quotient or root of finite values, so a value that is not
/// finite is one a double could not hold.
fn finite(v: f64) -> Result<f64, OneSampleCode> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(OneSampleCode::Overflow)
    }
}

/// A refusal from `stats_verify`, by the range fault it carries, in this
/// module's codes, as `stats_ttest` words it.
fn refused(fault: Option<RangeFault>) -> OneSampleCode {
    match fault {
        Some(RangeFault::Overflow) => OneSampleCode::Overflow,
        Some(RangeFault::Underflow) => OneSampleCode::Underflow,
        None => OneSampleCode::ArithmeticRefused,
    }
}

/// An sd from `stats_verify::sample_variance`.
fn sd_of(values: &[f64]) -> Result<f64, OneSampleCode> {
    finite(
        sample_variance(values)
            .map_err(|e| refused(RangeFault::of(&e)))?
            .sqrt(),
    )
}

/// The numbers of one one-sample run.
struct OneSampleNumbers {
    mean: f64,
    sd: f64,
    t: f64,
    df: f64,
    p: f64,
    mean_difference: f64,
    se: f64,
    ci: [f64; 2],
    d: f64,
}

/// Every number of a one-sample result. `values` are ascending and hold at
/// least two, checked before.
fn one_sample_numbers(values: &[f64], test_value: f64) -> Result<OneSampleNumbers, OneSampleCode> {
    if values[0] == values[values.len() - 1] {
        return Err(OneSampleCode::ValuesConstant);
    }
    let r = t_test_one_sample(values, test_value).map_err(|e| refused(RangeFault::of(&e)))?;
    if essentially_constant(r.se, r.mean_a.abs()) {
        return Err(OneSampleCode::ValuesEssentiallyConstant);
    }
    let p = two_sided_p(
        r.t,
        r.df,
        OneSampleCode::QuantileOutOfRange,
        OneSampleCode::ArithmeticRefused,
    )?;
    let mean_difference = finite(r.mean_a - test_value)?;
    let ci = interval_95(
        mean_difference,
        r.se,
        r.df,
        OneSampleCode::QuantileOutOfRange,
        OneSampleCode::Overflow,
    )?;
    let sd = sd_of(values)?;
    let d = finite(mean_difference / sd)?;
    Ok(OneSampleNumbers {
        mean: r.mean_a,
        sd,
        t: r.t,
        df: r.df,
        p,
        mean_difference,
        se: r.se,
        ci,
        d,
    })
}

/// The numbers of one paired run.
struct PairedNumbers {
    first: ColumnStats,
    second: ColumnStats,
    t: f64,
    df: f64,
    p: f64,
    mean_difference: f64,
    sd_differences: f64,
    se: f64,
    ci: [f64; 2],
    dz: f64,
}

/// Every number of a paired result. The pairs are sorted by their difference
/// and hold at least two, checked before. The differences are formed here as
/// `t_test_paired` forms them, first minus second, in the same order, so the
/// mean difference and the sd of the differences are of the values t is of.
fn paired_numbers(first: &[f64], second: &[f64]) -> Result<PairedNumbers, OneSampleCode> {
    let differences: Vec<f64> = first.iter().zip(second).map(|(x, y)| x - y).collect();
    if differences[0] == differences[differences.len() - 1] {
        return Err(OneSampleCode::DifferencesConstant);
    }
    let r = t_test_paired(first, second).map_err(|e| refused(RangeFault::of(&e)))?;
    // Finite here: `t_test_paired` refused a mean of these same differences
    // that was not.
    let mean_difference = finite(mean(&differences))?;
    let mean_abs = |v: &[f64]| mean(&v.iter().map(|x| x.abs()).collect::<Vec<f64>>());
    let scale = mean_difference
        .abs()
        .max(mean_abs(first))
        .max(mean_abs(second));
    if essentially_constant(r.se, scale) {
        return Err(OneSampleCode::DifferencesEssentiallyConstant);
    }
    let p = two_sided_p(
        r.t,
        r.df,
        OneSampleCode::QuantileOutOfRange,
        OneSampleCode::ArithmeticRefused,
    )?;
    let ci = interval_95(
        mean_difference,
        r.se,
        r.df,
        OneSampleCode::QuantileOutOfRange,
        OneSampleCode::Overflow,
    )?;
    let sd_differences = sd_of(&differences)?;
    let dz = finite(mean_difference / sd_differences)?;
    Ok(PairedNumbers {
        first: ColumnStats {
            mean: r.mean_a,
            sd: sd_of(first)?,
        },
        second: ColumnStats {
            mean: r.mean_b,
            sd: sd_of(second)?,
        },
        t: r.t,
        df: r.df,
        p,
        mean_difference,
        sd_differences,
        se: r.se,
        ci,
        dz,
    })
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

fn manifest_of<P: Serialize>(
    spec: &OperationSpec<P>,
    dataset: &Dataset,
    capability_id: CapabilityId,
    capability_version: u32,
    rows_used: u64,
    exclusions: Vec<Exclusion<OneSampleCode>>,
    output: Vec<u8>,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<OneSampleCode> {
    let parameters = serde_json::to_vec(&spec.parameters)
        .expect("parameters of aliases, numbers and words serialise");
    RunManifest {
        schema_version: SchemaVersion,
        capability_id,
        capability_version,
        engine_version: ENGINE_VERSION,
        inputs: spec.inputs.iter().map(|i| i.content_hash.clone()).collect(),
        parameters_hash: ContentHash::of_bytes(&parameters),
        seed: spec.random_seed,
        rows_read: dataset.rows() as u64,
        rows_used,
        exclusions,
        platform,
        elapsed_ms,
        output_hash: ContentHash::of_bytes(&output),
        reproducibility: Reproducibility::Bitwise,
    }
}

/// The record of a one-sample run: rows used are the values used, and the
/// rows left out are the result's exclusions, so the three add up. The output
/// hash is SHA-256 of [`OneSampleResult::canonical_bytes`].
pub fn one_sample_manifest(
    spec: &OperationSpec<OneSampleParams>,
    dataset: &Dataset,
    result: &OneSampleResult,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<OneSampleCode> {
    manifest_of(
        spec,
        dataset,
        one_sample_capability_id(),
        ONE_SAMPLE_VERSION,
        result.n,
        result.exclusions.clone(),
        result.canonical_bytes(),
        elapsed_ms,
        platform,
    )
}

/// The record of a paired run: rows used are the pairs used. The output hash
/// is SHA-256 of [`PairedResult::canonical_bytes`].
pub fn paired_manifest(
    spec: &OperationSpec<PairedParams>,
    dataset: &Dataset,
    result: &PairedResult,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<OneSampleCode> {
    manifest_of(
        spec,
        dataset,
        paired_capability_id(),
        PAIRED_VERSION,
        result.n_pairs,
        result.exclusions.clone(),
        result.canonical_bytes(),
        elapsed_ms,
        platform,
    )
}
