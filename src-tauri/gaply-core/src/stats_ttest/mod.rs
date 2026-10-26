//! **The independent-samples t-test as an analysis of the user's own data
//! (Research OS, Phase 4B-1): `stats.analyze.independent_t`.**
//!
//! Not a reproduction of a reported result: that is Level 1's
//! `stats.verify.independent_t`. This module computes the test on a numeric
//! outcome split by a two-level group, with no claim to compare against.
//!
//! * [`validate`] checks an `OperationSpec<TTestParams>` against the dataset.
//!   The variant is never chosen for the user: until the spec names Student or
//!   Welch, validation needs input (decision 64). A group with more than two
//!   levels needs the two chosen, by index (decision 65).
//! * [`run`] computes, after one check point: t, df, two-sided p, the mean
//!   difference with its standard error and 95% interval, Cohen's d and the
//!   standard deviation it divides by, each group's n, mean and sd, the sd
//!   ratio, the rows left out by reason, and the limitations.
//! * [`manifest`] records the run, and [`export`] writes the result as CSV
//!   and the manifest as JSON.
//!
//! Pure: no file, network, process, clock or model; the caller passes the
//! elapsed time and the platform (`tests/contracts_are_pure.rs`).
//!
//! **One implementation of each formula (decision 62).** t, df and the
//! standard error come from `stats_verify::t_test_student` and
//! `t_test_welch` (the standard error is their `se` field, the number t
//! divides by); p from `stats_l0::pvalue::t_two_tailed`; each group's mean and
//! variance from `stats_verify::mean` and `sample_variance`, given the values
//! in ascending order as `stats_desc` gives them, so permuting the rows changes
//! no bit of the result. Nothing here computes a pooled variance: Student's d
//! divides by the pooled sd recovered from Student's own standard error,
//! se / sqrt(1/n1 + 1/n2). Welch's d divides by sqrt((v1 + v2) / 2), the
//! standardizer of `effectsize::cohens_d(pooled_sd = FALSE)` (decision 63),
//! which no other module computes. The 95% quantile inverts
//! `pvalue::t_two_tailed` by bisection ([`t_quantile_975`]): statrs's
//! `StudentsT::inverse_cdf` missed R's `qt` by up to 6.8e-2 at large df
//! (`tests/ttest_quantile.rs` measures the shipped one against R).
//!
//! **The data (decision 65).** The outcome must hold numbers only. The group
//! is read as categorical with Level 1's reading of cells (`Cell::level_code`:
//! `1` and `1.0` are one level; text is trimmed), its levels sorted by code
//! point, as `stats_desc` sorts them. The spec names the two levels by index
//! in that order; with exactly two, they default to 0 and 1. Group 1 is the
//! first named, and the mean difference is group 1 minus group 2. A row is
//! left out for one reason, the first that applies: `missing_group`,
//! `other_level`, `missing_outcome`, `missing_code` (a value the user ticked,
//! applied to the outcome only, decision 38).
//!
//! **Edge cases are closed outcomes, never NaN or infinity.** A group with
//! fewer than two values is refused (`group_too_small`); both groups
//! constant, a standard error of exactly 0, is `both_groups_constant`; a
//! standard error that is not 0 but below 10 · ε · max(|mean 1|, |mean 2|),
//! spread lost in rounding, is `groups_essentially_constant`, R's "data are
//! essentially constant" (decision 259, [`essentially_constant`]); one
//! constant group is computed under both variants, as R does, and only its
//! sd ratio is `sd_ratio_undefined`; a sum, square or root a double cannot
//! hold is `overflow`, and a sum of squares that falls below the smallest
//! normal double although the values differ is `underflow`, each read from
//! `stats_verify::RangeFault` as the descriptive summary reads it (decision
//! 70); any other refusal of the arithmetic is `arithmetic_refused`.
//!
//! **Not assessed, and said so in every result:** normality
//! (`normality_not_assessed`); d has no interval
//! (`cohens_d_has_no_interval`: effectsize builds one with a noncentral t,
//! which Gaply does not have). The wording is neutral: no verdict.
//!
//! **Privacy.** `TTestResult` holds the two level labels, for the user's own
//! screen and the export the user saves; it derives `Serialize` for that and
//! its labels print as counts through `Debug`. The values themselves never
//! leave [`Split`], which derives nothing. The manifest holds hashes and
//! counts.

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
use crate::stats_l0::pvalue::t_two_tailed;
use crate::stats_l1::binding::suspect_codes_in;
use crate::stats_l1::dataset::{Cell, ColumnIssue, ColumnSummary, Dataset};
use crate::stats_verify::{mean, sample_variance, t_test_student, t_test_welch, RangeFault};

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.independent_t";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// The interval's coverage. Fixed in version 1.
pub const CONFIDENCE_LEVEL: f64 = 0.95;
/// The most levels a group column may have to be listed for the user to
/// choose two (decision 87): a column with more is a measurement, not a
/// grouping, and its values are never sent to the screen as a list. The one
/// constant; the screen reads it from `ttestLimits.json`, pinned to it by
/// `src-tauri/tests/stats_ttest_wire.rs`.
pub const MAX_GROUP_LEVELS: usize = 50;
/// The quantile the interval uses: 1 − (1 − 0.95) / 2.
const UPPER_TAIL_POINT: f64 = 0.975;

closed! {
    /// Every finding, outcome, exclusion and limitation of the t-test.
    pub enum TTestCode in "stats.ttest." {
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
        /// The outcome and the group are one column.
        SameColumn => "same_column",
        /// The outcome holds text. Params `column`, `count`.
        OutcomeHoldsText => "outcome_holds_text",
        /// The outcome holds NaN or an infinity. Params `column`, `count`.
        OutcomeNonFinite => "outcome_non_finite",
        /// The group has fewer than two levels. Params `column`, `count`.
        GroupOneLevel => "group_one_level",
        /// A level index is not one of the group's. Params `index`, `limit`
        /// (the highest index there is).
        LevelOutOfRange => "level_out_of_range",
        /// Both chosen levels are the same. Param `index`.
        LevelsEqual => "levels_equal",
        /// A missing code is NaN or an infinity. Param `index`.
        MissingCodeNotFinite => "missing_code_not_finite",
        /// Needs input: the user has not chosen Student or Welch.
        VariantNotChosen => "variant_not_chosen",
        /// Needs input: the group has more than two levels and two were not
        /// chosen. Param `count` (the levels there are).
        LevelsNotChosen => "levels_not_chosen",
        /// A candidate missing code the rule found in the outcome and the user
        /// did not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Refused: a group has fewer than two values after the exclusions.
        /// Params `index` (the level), `count`.
        GroupTooSmall => "group_too_small",
        /// Not computable: both groups are constant, so the standard error is
        /// exactly 0.
        BothGroupsConstant => "both_groups_constant",
        /// Not computable: a sum, square, quotient or root a double cannot
        /// hold (decision 70).
        Overflow => "overflow",
        /// Not computable: a sum of squares underflowed though the values
        /// differ (decision 70).
        Underflow => "underflow",
        /// Not computable: the arithmetic refused for a reason it did not
        /// name (no p for the statistic, or a refusal that is not a range
        /// fault).
        ArithmeticRefused => "arithmetic_refused",
        /// Not computable: the interval's quantile is not measured at this
        /// many degrees of freedom (above `QUANTILE_MAX_DF`).
        QuantileOutOfRange => "quantile_out_of_range",
        /// The sd ratio is not defined: a group's sd is 0.
        SdRatioUndefined => "sd_ratio_undefined",
        /// Excluded: the row's group is empty or a missing-value token.
        MissingGroup => "missing_group",
        /// Excluded: the row's group is not one of the two chosen levels.
        OtherLevel => "other_level",
        /// Excluded: the row's outcome is empty or a missing-value token.
        MissingOutcome => "missing_outcome",
        /// Excluded: the row's outcome is a value the user listed.
        MissingCode => "missing_code",
        /// Limitation: normality is not assessed (decision 64).
        NormalityNotAssessed => "normality_not_assessed",
        /// Limitation: Cohen's d has no confidence interval (decision 63).
        CohensDHasNoInterval => "cohens_d_has_no_interval",
        /// A column has more levels than a group may (decision 87): its
        /// levels are not listed, and `validate` and `run` refuse it as the
        /// group (decision 94). Params `count`, `limit`
        /// ([`MAX_GROUP_LEVELS`]).
        TooManyLevels => "too_many_levels",
        /// Not computable: the standard error is not 0 but below 10 · ε ·
        /// max(|mean 1|, |mean 2|), so the groups' spread is lost in rounding
        /// and t means nothing; R stops on the same rule (decision 259).
        GroupsEssentiallyConstant => "groups_essentially_constant",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// Which t-test. The user chooses; nothing here chooses for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Variant {
    /// Pooled variance: assumes the two variances are equal.
    Student,
    /// Welch–Satterthwaite: does not.
    Welch,
}

/// The t-test's parameters. `levels` names the two levels by their index in
/// code-point order, group 1 first; `None` is allowed only when the group has
/// exactly two. `variant` must be chosen before the spec can run.
/// `missing_codes` apply to the outcome only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TTestParams {
    pub outcome: Alias,
    pub group: Alias,
    pub levels: Option<[u32; 2]>,
    pub variant: Option<Variant>,
    pub missing_codes: Vec<f64>,
}

/* ------------------------------------------------------------------------ *
 *  The result
 * ------------------------------------------------------------------------ */

/// The standard deviation Cohen's d divides by (decision 63).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Standardizer {
    /// Student: the pooled sd.
    PooledSd,
    /// Welch: sqrt((v1 + v2) / 2), as `effectsize::cohens_d(pooled_sd = FALSE)`.
    RootMeanVariance,
}

/// A number, or the closed reason it is not defined.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Value(f64),
    NotComputable(TTestCode),
}

/// One group: its level's index and label, and what it holds after the
/// exclusions. The label is the user's own text.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSummary {
    pub level: u32,
    pub label: String,
    pub n: u64,
    pub mean: f64,
    pub sd: f64,
}

impl std::fmt::Debug for GroupSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroupSummary")
            .field("level", &self.level)
            .field("label_chars", &self.label.chars().count())
            .field("n", &self.n)
            .finish_non_exhaustive()
    }
}

/// The t-test's result. Every number is finite.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TTestResult {
    pub variant: Variant,
    pub t: f64,
    pub df: f64,
    /// Two-sided.
    pub p: f64,
    /// Group 1's mean minus group 2's.
    pub mean_difference: f64,
    /// The standard error t divides by.
    pub standard_error: f64,
    pub confidence_level: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
    pub cohens_d: f64,
    pub d_standardizer: Standardizer,
    /// The standard deviation d divides by.
    pub d_standardizer_value: f64,
    /// Group 1, then group 2.
    pub groups: [GroupSummary; 2],
    /// The larger sd over the smaller. Information, never a recommendation
    /// (decision 64).
    pub sd_ratio: Stat,
    /// The rows left out, by reason; reasons with no row are not listed.
    pub exclusions: Vec<Exclusion<TTestCode>>,
    pub limitations: Vec<TTestCode>,
}

impl TTestResult {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, closed words and strings serialises")
    }
}

/// What every result says it did not assess.
const LIMITATIONS: [TTestCode; 2] = [
    TTestCode::NormalityNotAssessed,
    TTestCode::CohensDHasNoInterval,
];

/* ------------------------------------------------------------------------ *
 *  The one splitting pass
 * ------------------------------------------------------------------------ */

/// Each level of a column read as categorical (Level 1's reading of cells,
/// decision 17) and its non-missing cells, in code-point order: THE level
/// function, which [`levels_of`] and [`group_levels`] both read.
fn level_counts(cells: &[Cell]) -> BTreeMap<String, u64> {
    let mut levels: BTreeMap<String, u64> = BTreeMap::new();
    for cell in cells {
        if let Some(code) = cell.level_code() {
            *levels.entry(code).or_insert(0) += 1;
        }
    }
    levels
}

/// The group column's levels, in code-point order.
fn levels_of(cells: &[Cell]) -> Vec<String> {
    level_counts(cells).into_keys().collect()
}

/// A column's levels as a spec's `levels` indexes them: position `i` here is
/// index `i` (Research OS, Phase 4B-2 follow-up, decision 81), with its
/// count of non-missing cells. Any column, numeric ones included, read as
/// categorical. `None` when no column has that alias. The labels are the
/// user's own; the app shows them on the user's screen only.
pub fn group_levels(dataset: &Dataset, group: Alias) -> Option<Vec<(String, u64)>> {
    let (_, cells) = column(dataset, group)?;
    Some(level_counts(cells).into_iter().collect())
}

/// The outcome's values in the two groups, each in ascending order, and the
/// rows left out. Never leaves this module, and derives nothing that could
/// print or serialise it.
struct Split {
    values: [Vec<f64>; 2],
    /// `missing_group`, `other_level`, `missing_outcome`, `missing_code`.
    excluded: [u64; 4],
}

const EXCLUSION_CODES: [TTestCode; 4] = [
    TTestCode::MissingGroup,
    TTestCode::OtherLevel,
    TTestCode::MissingOutcome,
    TTestCode::MissingCode,
];

fn split(outcome: &[Cell], group: &[Cell], chosen: [&str; 2], codes: &[f64]) -> Split {
    let mut s = Split {
        values: [Vec::new(), Vec::new()],
        excluded: [0; 4],
    };
    for (y, g) in outcome.iter().zip(group) {
        let Some(level) = g.level_code() else {
            s.excluded[0] += 1;
            continue;
        };
        let Some(k) = chosen.iter().position(|c| *c == level) else {
            s.excluded[1] += 1;
            continue;
        };
        match y {
            Cell::Missing => s.excluded[2] += 1,
            Cell::Number(v) if codes.contains(v) => s.excluded[3] += 1,
            Cell::Number(v) => s.values[k].push(*v),
            // Refused by validation before any split is used.
            Cell::Text(_) => s.excluded[2] += 1,
        }
    }
    for v in &mut s.values {
        v.sort_by(f64::total_cmp);
    }
    s
}

/* ------------------------------------------------------------------------ *
 *  validate
 * ------------------------------------------------------------------------ */

/// The capability's id, parsed.
pub fn capability_id() -> CapabilityId {
    CapabilityId::parse(CAPABILITY_ID).expect("the capability id is well formed")
}

/// A spec for this capability on this dataset.
pub fn spec(
    operation_id: OperationId,
    dataset: &Dataset,
    parameters: TTestParams,
) -> OperationSpec<TTestParams> {
    OperationSpec {
        schema_version: SchemaVersion,
        operation_id,
        workspace_id: WorkspaceId::Statistics,
        capability_id: capability_id(),
        capability_version: CAPABILITY_VERSION,
        inputs: vec![input_ref(dataset)],
        parameters,
        random_seed: None,
        confirmation_refs: Vec::new(),
    }
}

pub(crate) fn column(dataset: &Dataset, alias: Alias) -> Option<(&ColumnSummary, &[Cell])> {
    Some((
        dataset.schema.column(alias.0)?,
        dataset.column_cells(alias.0)?,
    ))
}

fn column_param(alias: Alias) -> ParamValue<TTestCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

pub(crate) fn non_finite_cells(column: &ColumnSummary) -> u64 {
    column
        .issues
        .iter()
        .map(|i| {
            if let ColumnIssue::NonFinite { cells } = i {
                *cells as u64
            } else {
                0
            }
        })
        .sum()
}

/// What the parameters leave wrong or open on this dataset.
struct Checked<'d> {
    errors: Vec<Diagnostic<TTestCode>>,
    questions: Vec<Diagnostic<TTestCode>>,
    /// The outcome's and the group's cells, the group's levels and the two
    /// chosen indices, when the columns and the levels are good.
    ready: Option<Ready<'d>>,
}

struct Ready<'d> {
    outcome: &'d [Cell],
    group: &'d [Cell],
    levels: Vec<String>,
    chosen: [u32; 2],
}

impl Ready<'_> {
    fn split(&self, codes: &[f64]) -> Split {
        let chosen = self.chosen.map(|i| self.levels[i as usize].as_str());
        split(self.outcome, self.group, chosen, codes)
    }
}

/// Shared by `validate` and `run`: the errors in a fixed order (missing
/// codes, then the outcome, the group, the levels, then the group sizes), and
/// the open questions (levels, then the variant).
fn check<'d>(dataset: &'d Dataset, params: &TTestParams) -> Checked<'d> {
    let mut errors = Vec::new();
    let mut questions = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(
                Diagnostic::error(TTestCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64),
            );
        }
    }
    let outcome = column(dataset, params.outcome);
    let group = column(dataset, params.group);
    for (alias, found) in [
        (params.outcome, outcome.is_some()),
        (params.group, group.is_some()),
    ] {
        if !found {
            errors.push(
                Diagnostic::error(TTestCode::UnknownColumn)
                    .with(ParamKey::Column, column_param(alias)),
            );
        }
    }
    let mut ready = None;
    if let (Some((o_summary, o_cells)), Some((_, g_cells))) = (outcome, group) {
        if params.outcome == params.group {
            errors.push(Diagnostic::error(TTestCode::SameColumn));
        } else {
            let non_finite = non_finite_cells(o_summary);
            let texts = o_cells
                .iter()
                .filter(|c| matches!(c, Cell::Text(_)))
                .count() as i64;
            if non_finite > 0 {
                errors.push(
                    Diagnostic::error(TTestCode::OutcomeNonFinite)
                        .with(ParamKey::Column, column_param(params.outcome))
                        .int(ParamKey::Count, non_finite as i64),
                );
            } else if texts > 0 {
                errors.push(
                    Diagnostic::error(TTestCode::OutcomeHoldsText)
                        .with(ParamKey::Column, column_param(params.outcome))
                        .int(ParamKey::Count, texts),
                );
            }
            let levels = levels_of(g_cells);
            let k = levels.len() as u32;
            let mut chosen = None;
            if k < 2 {
                errors.push(
                    Diagnostic::error(TTestCode::GroupOneLevel)
                        .with(ParamKey::Column, column_param(params.group))
                        .int(ParamKey::Count, i64::from(k)),
                );
            } else if levels.len() > MAX_GROUP_LEVELS {
                // Decision 94: decision 87's limit is the capability's, so it
                // holds for a spec built anywhere, not only by the picker.
                errors.push(
                    Diagnostic::error(TTestCode::TooManyLevels)
                        .int(ParamKey::Count, i64::from(k))
                        .int(ParamKey::Limit, MAX_GROUP_LEVELS as i64),
                );
            } else {
                match params.levels {
                    Some([a, b]) => {
                        let mut ok = true;
                        for i in [a, b] {
                            if i >= k {
                                ok = false;
                                errors.push(
                                    Diagnostic::error(TTestCode::LevelOutOfRange)
                                        .int(ParamKey::Index, i64::from(i))
                                        .int(ParamKey::Limit, i64::from(k - 1)),
                                );
                            }
                        }
                        if a == b {
                            ok = false;
                            errors.push(
                                Diagnostic::error(TTestCode::LevelsEqual)
                                    .int(ParamKey::Index, i64::from(a)),
                            );
                        }
                        if ok {
                            chosen = Some([a, b]);
                        }
                    }
                    None if k == 2 => chosen = Some([0, 1]),
                    None => questions.push(
                        Diagnostic::note(TTestCode::LevelsNotChosen)
                            .int(ParamKey::Count, i64::from(k)),
                    ),
                }
            }
            if let Some(chosen) = chosen {
                ready = Some(Ready {
                    outcome: o_cells,
                    group: g_cells,
                    levels,
                    chosen,
                });
            }
        }
    }
    // Group sizes only once nothing else is wrong: they are counted under the
    // codes and over the chosen levels.
    if errors.is_empty() {
        if let Some(r) = &ready {
            let s = r.split(&params.missing_codes);
            for (k, values) in s.values.iter().enumerate() {
                if values.len() < 2 {
                    errors.push(
                        Diagnostic::error(TTestCode::GroupTooSmall)
                            .int(ParamKey::Index, i64::from(r.chosen[k]))
                            .int(ParamKey::Count, values.len() as i64),
                    );
                }
            }
        }
    }
    if params.variant.is_none() {
        questions.push(Diagnostic::note(TTestCode::VariantNotChosen));
    }
    Checked {
        errors,
        questions,
        ready,
    }
}

/// Checks the spec against the dataset. Unsupported when it names another
/// capability, version or workspace; invalid on a wrong input or a parameter
/// error; needs input while the variant, or the two of more than two levels,
/// is not chosen; a warning for a candidate missing code in the outcome that
/// the user did not list.
pub fn validate(
    dataset: &Dataset,
    spec: &OperationSpec<TTestParams>,
) -> ValidationReport<TTestCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID
    {
        unsupported.push(Diagnostic::error(TTestCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(TTestCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(TTestCode::InputMismatch));
            }
        }
        inputs => errors.push(
            Diagnostic::error(TTestCode::InputCount).int(ParamKey::Count, inputs.len() as i64),
        ),
    }
    let params = &spec.parameters;
    let checked = check(dataset, params);
    errors.extend(checked.errors);

    let mut warnings = Vec::new();
    if let Some((_, cells)) = column(dataset, params.outcome) {
        for s in suspect_codes_in(params.outcome.0, cells) {
            if !params.missing_codes.contains(&f64::from(s.code)) {
                warnings.push(
                    Diagnostic::warning(TTestCode::SuspectMissingCode)
                        .with(ParamKey::Column, column_param(params.outcome))
                        .int(ParamKey::Value, i64::from(s.code))
                        .int(ParamKey::Count, s.cells as i64),
                );
            }
        }
    }
    let rows = dataset.rows() as u64;
    let estimate = ResourceEstimate {
        rows,
        columns: 2,
        cells: rows.saturating_mul(2),
    };
    ValidationReport::new(unsupported, errors, checked.questions, warnings, estimate)
}

/* ------------------------------------------------------------------------ *
 *  run
 * ------------------------------------------------------------------------ */

/// Why [`run`] returned no result.
#[derive(Debug, Clone, PartialEq)]
pub enum Halt<B> {
    /// The parameters are wrong or open for this dataset, or the result is
    /// not computable: what [`compute`] returns.
    Refused(Diagnostic<TTestCode>),
    /// The caller's check point stopped the run before the arithmetic.
    Stopped { reason: B },
}

/// [`run`] with a check point that never stops.
pub fn compute(
    dataset: &Dataset,
    params: &TTestParams,
) -> Result<TTestResult, Diagnostic<TTestCode>> {
    match run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The t-test, with the same check point as `stats_desc::describe_until`:
/// `checkpoint(0)` is called once, after the parameters are checked and
/// before the arithmetic, and a `Break` stops the run there. What a stop
/// means is the caller's: this module reads no flag and no clock.
pub fn run<B>(
    dataset: &Dataset,
    params: &TTestParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<TTestResult, Halt<B>> {
    let checked = check(dataset, params);
    if let Some(e) = checked.errors.into_iter().next() {
        return Err(Halt::Refused(e));
    }
    if let Some(q) = checked.questions.into_iter().next() {
        return Err(Halt::Refused(q));
    }
    let ready = checked
        .ready
        .expect("no error and no question means the levels are chosen");
    let variant = params
        .variant
        .expect("no question means the variant is chosen");
    if let ControlFlow::Break(reason) = checkpoint(0) {
        return Err(Halt::Stopped { reason });
    }
    let s = ready.split(&params.missing_codes);
    let groups = arithmetic(&s.values, variant).map_err(|c| Halt::Refused(Diagnostic::error(c)))?;
    let exclusions = EXCLUSION_CODES
        .iter()
        .zip(s.excluded)
        .filter(|(_, n)| *n > 0)
        .map(|(code, count)| {
            let alias = match code {
                TTestCode::MissingGroup | TTestCode::OtherLevel => params.group,
                _ => params.outcome,
            };
            Exclusion {
                code: *code,
                count,
                params: BTreeMap::from([(ParamKey::Column, column_param(alias))]),
            }
        })
        .collect();
    let summary = |k: usize| GroupSummary {
        level: ready.chosen[k],
        label: ready.levels[ready.chosen[k] as usize].clone(),
        n: s.values[k].len() as u64,
        mean: groups.means[k],
        sd: groups.sds[k],
    };
    Ok(TTestResult {
        variant,
        t: groups.t,
        df: groups.df,
        p: groups.p,
        mean_difference: groups.mean_difference,
        standard_error: groups.se,
        confidence_level: CONFIDENCE_LEVEL,
        ci_lower: groups.ci[0],
        ci_upper: groups.ci[1],
        cohens_d: groups.d,
        d_standardizer: groups.standardizer,
        d_standardizer_value: groups.sd_used,
        groups: [summary(0), summary(1)],
        sd_ratio: groups.sd_ratio,
        exclusions,
        limitations: LIMITATIONS.to_vec(),
    })
}

/// The numbers of one run, before the labels are attached.
struct Numbers {
    t: f64,
    df: f64,
    p: f64,
    mean_difference: f64,
    se: f64,
    ci: [f64; 2],
    d: f64,
    standardizer: Standardizer,
    sd_used: f64,
    means: [f64; 2],
    sds: [f64; 2],
    sd_ratio: Stat,
}

/// `v`, or `overflow` when it is infinite or NaN: every number here is a
/// sum, product, quotient or root of finite values, so a value that is not
/// finite is one a double could not hold.
fn finite(v: f64) -> Result<f64, TTestCode> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(TTestCode::Overflow)
    }
}

/// A refusal from `stats_verify`, by the range fault it carries
/// (`RangeFault::of`), in this module's codes, as `stats_desc` words it.
fn refused(fault: Option<RangeFault>) -> TTestCode {
    match fault {
        Some(RangeFault::Overflow) => TTestCode::Overflow,
        Some(RangeFault::Underflow) => TTestCode::Underflow,
        None => TTestCode::ArithmeticRefused,
    }
}

/// The bracket the quantile is searched in. At df = 1 the two-sided p of
/// 1e3 is about 6.4e-4, below 0.05, so every df of 1 or more has its root
/// inside.
const QUANTILE_BRACKET: [f64; 2] = [0.0, 1e3];
/// More halvings than a width of 1e3 needs to reach one unit in the last
/// place (about 62); the loop stops earlier, when the midpoint is an end.
const QUANTILE_MAX_HALVINGS: u32 = 200;
/// The largest df the quantile is measured to meet its gate at (5.2e-13 of
/// R's `qt` at 1e4). Above it the root inherits `pvalue::t_two_tailed`'s own
/// error at large df (1.8e-11 at 1e5, 3.6e-9 at 1e7), so the quantile is
/// refused rather than given. Unreachable through the ingest: 5,000 rows
/// give at most df 4,998.
pub const QUANTILE_MAX_DF: f64 = 1e4;

/// The upper 97.5% point of Student's t with `df` degrees of freedom: the q
/// whose two-sided p, `stats_l0::pvalue::t_two_tailed(q, df)`, is 0.05, found
/// by bisection on [0, 1e3] (decision 62's fallback). statrs's own
/// `StudentsT::inverse_cdf` was measured first and did not meet the gate:
/// within 6.1e-13 of R's `qt` up to df 4,998, but 3.5e-6 off at df 1e5 and
/// 6.8e-2 at 1e7 (Phase 4B-1 report). `None` for a df that is not finite and
/// above 0, above [`QUANTILE_MAX_DF`], or whose root is not in the bracket.
/// Measured against R's `qt` in `tests/ttest_quantile.rs`.
pub fn t_quantile_975(df: f64) -> Option<f64> {
    if !(df.is_finite() && df > 0.0 && df <= QUANTILE_MAX_DF) {
        return None;
    }
    let target = 2.0 * (1.0 - UPPER_TAIL_POINT);
    let p = |q: f64| t_two_tailed(q, df);
    let [mut lo, mut hi] = QUANTILE_BRACKET;
    // p falls as q rises: above the target at lo, below it at hi.
    if !(p(lo)? > target && p(hi)? < target) {
        return None;
    }
    for _ in 0..QUANTILE_MAX_HALVINGS {
        let mid = lo + (hi - lo) / 2.0;
        if mid == lo || mid == hi {
            break;
        }
        if p(mid)? > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    // The end whose p is nearer the target; the lower one on a tie.
    let q = if (p(hi)? - target).abs() < (p(lo)? - target).abs() {
        hi
    } else {
        lo
    };
    (q.is_finite() && q > 0.0).then_some(q)
}

/// The 95% interval `estimate ± q · se`, q being [`t_quantile_975`] at `df`:
/// THE interval of the t family (Research OS, Phase 8D-1, decision 254),
/// shared with `stats_onesample` and generic over the caller's codes, so this
/// module gains no type. `out_of_range` when the quantile is not measured at
/// `df`; `overflow` when q · se, or either end, is not finite. No `mul_add`:
/// the two ends are `estimate − half` and `estimate + half`, as before.
pub(crate) fn interval_95<C: Copy>(
    estimate: f64,
    se: f64,
    df: f64,
    out_of_range: C,
    overflow: C,
) -> Result<[f64; 2], C> {
    let finite = |v: f64| if v.is_finite() { Ok(v) } else { Err(overflow) };
    let q = t_quantile_975(df).ok_or(out_of_range)?;
    let half = finite(q * se)?;
    Ok([finite(estimate - half)?, finite(estimate + half)?])
}

/// Whether a standard error is rounding rather than spread: `se` below
/// 10 · ε · `scale`, ε being `f64::EPSILON`. THE "essentially constant" rule
/// of the t family (Research OS, Phase 8D-1b, decision 259), shared with
/// `stats_onesample`; each caller passes its own scale. It is R's rule, in
/// `t.test.default` (R's file src/library/stats/R/t.test.R, the one-sample
/// and paired check at lines 62 to 64 and the two-sample check at line 94 of
/// trunk, https://raw.githubusercontent.com/wch/r-source/trunk/src/library/stats/R/t.test.R,
/// read 26 Oct 2026, and the same in R 4.6.1 as installed), where R stops
/// with "data are essentially constant". Callers check it after the exact-0
/// checks and after `stats_verify`'s own refusals, so a standard error of
/// exactly 0 keeps its own code.
pub(crate) fn essentially_constant(se: f64, scale: f64) -> bool {
    se < 10.0 * f64::EPSILON * scale
}

/// Every number of the result. `values` are ascending and each holds at
/// least two, checked before.
fn arithmetic(values: &[Vec<f64>; 2], variant: Variant) -> Result<Numbers, TTestCode> {
    let [a, b] = values;
    let constant = |v: &[f64]| v[0] == v[v.len() - 1];
    if constant(a) && constant(b) {
        return Err(TTestCode::BothGroupsConstant);
    }
    let r = match variant {
        Variant::Student => t_test_student(a, b),
        Variant::Welch => t_test_welch(a, b),
    }
    .map_err(|e| refused(RangeFault::of(&e)))?;
    if essentially_constant(r.se, r.mean_a.abs().max(r.mean_b.abs())) {
        return Err(TTestCode::GroupsEssentiallyConstant);
    }
    let p = t_two_tailed(r.t, r.df).ok_or(TTestCode::ArithmeticRefused)?;
    let mean_difference = finite(r.mean_a - r.mean_b)?;
    let ci = interval_95(
        mean_difference,
        r.se,
        r.df,
        TTestCode::QuantileOutOfRange,
        TTestCode::Overflow,
    )?;
    let means = [finite(mean(a))?, finite(mean(b))?];
    let variances = [
        sample_variance(a).map_err(|e| refused(RangeFault::of(&e)))?,
        sample_variance(b).map_err(|e| refused(RangeFault::of(&e)))?,
    ];
    let sds = [finite(variances[0].sqrt())?, finite(variances[1].sqrt())?];
    let (standardizer, sd_used) = match variant {
        // The pooled sd from Student's own standard error, which is
        // sp · sqrt(1/n1 + 1/n2): no second pooled variance.
        Variant::Student => {
            let (n1, n2) = (a.len() as f64, b.len() as f64);
            (Standardizer::PooledSd, r.se / (1.0 / n1 + 1.0 / n2).sqrt())
        }
        Variant::Welch => (
            Standardizer::RootMeanVariance,
            ((variances[0] + variances[1]) / 2.0).sqrt(),
        ),
    };
    let sd_used = finite(sd_used)?;
    let d = finite(mean_difference / sd_used)?;
    let (lo, hi) = if sds[0] <= sds[1] {
        (sds[0], sds[1])
    } else {
        (sds[1], sds[0])
    };
    let sd_ratio = if lo == 0.0 {
        Stat::NotComputable(TTestCode::SdRatioUndefined)
    } else {
        match finite(hi / lo) {
            Ok(v) => Stat::Value(v),
            Err(c) => Stat::NotComputable(c),
        }
    };
    Ok(Numbers {
        t: r.t,
        df: r.df,
        p,
        mean_difference,
        se: r.se,
        ci,
        d,
        standardizer,
        sd_used,
        means,
        sds,
        sd_ratio,
    })
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows
/// read are the data rows; rows used are the two groups' n; the rows left out
/// are the result's exclusions, so the three add up. The output hash is
/// SHA-256 of [`TTestResult::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<TTestParams>,
    dataset: &Dataset,
    result: &TTestResult,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<TTestCode> {
    let parameters = serde_json::to_vec(&spec.parameters)
        .expect("parameters of aliases, indices, words and numbers serialise");
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        inputs: spec.inputs.iter().map(|i| i.content_hash.clone()).collect(),
        parameters_hash: ContentHash::of_bytes(&parameters),
        seed: spec.random_seed,
        rows_read: dataset.rows() as u64,
        rows_used: result.groups.iter().map(|g| g.n).sum(),
        exclusions: result.exclusions.clone(),
        platform,
        elapsed_ms,
        output_hash: ContentHash::of_bytes(&result.canonical_bytes()),
        reproducibility: Reproducibility::Bitwise,
    }
}
