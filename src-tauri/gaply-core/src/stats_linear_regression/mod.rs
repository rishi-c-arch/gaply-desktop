//! **Ordinary least squares linear regression as an analysis of the user's own
//! data (Research OS, Phase 9A-1, decisions 669 to 671):
//! `stats.analyze.linear_regression`.**
//!
//! One numeric outcome and one to [`MAX_PREDICTORS`] numeric predictors, entered
//! together and listwise, as SPSS's REGRESSION does with method Enter and one
//! block. The model is y = b0 + b1 x1 + … + bk xk, with an intercept always.
//!
//! **One fit, one place (decision 670).** The coefficients, R⁻¹, the residuals,
//! RSS and MSS come from `stats_verify::ols_qr_parts`, the Householder QR that
//! Level 1's `ols_qr` wraps, with LINPACK `dqrdc2`'s rank test at R's tolerance
//! 1e-7. With n the complete rows, k the predictors and df = n − k − 1:
//!
//! * σ² = RSS / df and the standard error of the estimate σ; each coefficient's
//!   standard error √(σ² Σ_m (R⁻¹)²_jm), the sum over row j of R⁻¹, as `ols_qr`
//!   computes it; t = b / SE; its p, `stats_ttest::two_sided_p`; its 95%
//!   interval, `stats_ttest::interval_95`, both over `stats_dist`;
//! * R² = MSS / (MSS + RSS), adjusted R² = 1 − (1 − R²)(n − 1) / df, as `ols_qr`
//!   computes them, and R = √R²;
//! * the analysis of variance: regression SS = MSS on k df, residual SS = RSS on
//!   df, total SS = MSS + RSS on n − 1 df; F = (MSS / k) / σ², its p
//!   `stats_dist::f_upper(F, k, df)`;
//! * each column's mean (`stats_verify::mean`) and standard deviation (the root of
//!   `stats_verify::sample_variance`) over the complete rows;
//! * each predictor's standardized coefficient b_j sd_j / sd_y; its zero-order
//!   correlation with the outcome (`stats_verify::pearson_clamped`); its partial
//!   correlation t_j / √(t_j² + df) and part correlation t_j √((RSS / (MSS + RSS))
//!   / df); its VIF, SS_j s_j, SS_j = (n − 1) var_j its centred sum of squares and
//!   s_j the sum of the squares of row j of R⁻¹, and its tolerance 1 / VIF. With
//!   one predictor both are exactly 1, as SPSS prints them;
//! * the Durbin-Watson statistic Σ_{t ≥ 2} (e_t − e_{t−1})² / RSS over the
//!   complete rows in the file's order;
//! * the residuals' summary, as SPSS's Residuals Statistics: the predicted values'
//!   and the residuals' minimum, maximum, mean and standard deviation (n − 1), and
//!   the standardized predicted values' and standardized residuals' (e / σ)
//!   minimum and maximum.
//!
//! **Edge cases are decided from the values, never from an error's text.** RSS
//! exactly 0 is a perfect fit: σ, the residual mean square, every SE, t, p and
//! interval, F and its p, the partial and part correlations, the Durbin-Watson
//! statistic and the standardized residuals are not computable (`perfect_fit`);
//! R² and adjusted R² are 1. A p or quantile `stats_dist` does not measure is
//! `distribution_out_of_range`. A value too large for a double is `overflow`; a
//! statistic whose own value is below the smallest normal double, so that it
//! cannot hold its precision (σ², a mean square, an SE, t, F), is `too_small`.
//! Where only an intermediate would be subnormal or overflow while the statistic
//! itself is an ordinary double, it is computed in a form whose intermediates stay
//! normal: σ as √RSS / √df, an SE as σ √s_j, F as (MSS / RSS)(df / k), a standard
//! deviation by a scaled sum (decision 683). Two exceptions, stated rather than handled:
//! an s_j that is itself subnormal withholds its coefficient's five and its VIF and
//! tolerance as `too_small`, though σ √s_j could be normal (reachable only with one
//! predictor whose centred sum of squares lies between about 4.5e307 and 1.8e308); and
//! the split F's quotient MSS / RSS is not itself guarded, which the review of 9A-2
//! judged unreachable, since MSS is exactly 0 or far above RSS's rounding. Predicted values whose spread is at the
//! level of their own rounding cannot be standardized: `predicted_no_spread`
//! (decision 683's rule, `no_spread`). Each statistic by itself; never a refusal of
//! the run. The run is refused only by its checks and
//! by its two refusals: `predictors_collinear`, naming the first predictor the
//! rank test stops at (where R's `lm` gives NA), and `fit_out_of_range`, when the
//! fit's sums of squares, or any other `stats_verify` call of the run, cannot be
//! held in a double. The note `essentially_perfect_fit` follows R 4.6.1's
//! `summary.lm` (read in this session, decision 669): it warns when the residual
//! variance is finite and below (mean(fitted)² + var(fitted)) · 1e-30.
//!
//! **The data.** A row is used only when the outcome and every predictor have a
//! number that is not a ticked code. A row left out is counted once, under the
//! first column without one, the outcome first and then the predictors in the
//! list's order, as the related rank tests' are. Any column may be a scale score.
//!
//! **Pure** (`tests/contracts_are_pure.rs`): no file, network, process, clock or
//! model. The run's one check point is the caller's, before the arithmetic.
//!
//! **Privacy.** The result names columns by alias only and holds no label and no
//! cell. The values never leave [`Rows`], which derives nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::ops::ControlFlow;

use serde::{Deserialize, Serialize};

use crate::contracts::codes::closed;
use crate::contracts::{
    CapabilityId, ContentHash, Diagnostic, Exclusion, OperationId, OperationSpec, ParamKey,
    ParamValue, Platform, Reproducibility, ResourceEstimate, RunManifest, SchemaVersion,
    ValidationReport, WorkspaceId,
};
use crate::stats_desc::{
    input_ref, preparation_holds, score_code_count, suspect_codes_unless_score, Alias,
};
use crate::stats_dist::f_upper;
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_ttest::{column, interval_95, non_finite_cells, two_sided_p};
use crate::stats_verify::{
    mean, ols_qr_parts, pearson_clamped, sample_variance, QrFit, QrRefusal,
};

pub mod export;

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.linear_regression";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// The most predictors a run takes.
pub const MAX_PREDICTORS: usize = 50;
/// `summary.lm`'s factor in its "essentially perfect fit" rule (R 4.6.1).
pub const ESSENTIALLY_PERFECT_FACTOR: f64 = 1e-30;

closed! {
    /// Every refusal, outcome, exclusion, note and limitation of the linear
    /// regression.
    pub enum LinearRegressionCode in "stats.linear_regression." {
        /// The spec names another capability.
        UnknownCapability => "unknown_capability",
        /// The spec was written for another version of this capability.
        /// Params `expected`, `found`.
        CapabilityVersionMismatch => "capability_version_mismatch",
        /// The spec does not name exactly one input. Param `count`.
        InputCount => "input_count",
        /// The spec's input is not this dataset.
        InputMismatch => "input_mismatch",
        /// The spec's preparation does not describe this table.
        PreparationMismatch => "preparation_mismatch",
        /// A missing code is NaN or an infinity. Param `index`.
        MissingCodeNotFinite => "missing_code_not_finite",
        /// No predictor is chosen.
        NoPredictors => "no_predictors",
        /// More predictors than a run takes. Params `count`, `limit`.
        TooManyPredictors => "too_many_predictors",
        /// A column is named twice, among the predictors or the outcome among
        /// them. Param `column`, the alias named the second time.
        ColumnRepeated => "column_repeated",
        /// No column has that alias. Param `column`.
        UnknownColumn => "unknown_column",
        /// A column holds NaN or an infinity. Params `column`, `count`.
        ColumnNonFinite => "column_non_finite",
        /// A column holds text. Params `column`, `count`.
        ColumnHoldsText => "column_holds_text",
        /// A ticked missing code equals a value in a chosen score. Params
        /// `column`, `count`.
        CodeEqualsScore => "code_equals_score",
        /// No row has a number that is not a code in every chosen column.
        NoCompleteRows => "no_complete_rows",
        /// Fewer complete rows than the predictors and two. Params `count` (the
        /// rows), `limit` (the predictors and two).
        TooFewRows => "too_few_rows",
        /// The outcome has one value over the complete rows.
        OutcomeConstant => "outcome_constant",
        /// A predictor has one value over the complete rows. Param `column`, the
        /// first such in the list's order.
        PredictorConstant => "predictor_constant",
        /// A predictor is, to R's tolerance, a combination of the intercept and
        /// the predictors before it, where R's `lm` gives NA. Param `column`, the
        /// first such in the list's order.
        PredictorsCollinear => "predictors_collinear",
        /// The fit's sums of squares, or a mean, variance or correlation of the
        /// complete rows, cannot be held in a double.
        FitOutOfRange => "fit_out_of_range",
        /// A candidate missing code the rule found in a column and the user did
        /// not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Not computable: every residual is exactly zero.
        PerfectFit => "perfect_fit",
        /// Not computable: the distribution is not measured at these degrees of
        /// freedom.
        DistributionOutOfRange => "distribution_out_of_range",
        /// Not computable: a value along the way too large for a double (decision
        /// 683: too large only; a value too small is `too_small`).
        Overflow => "overflow",
        /// Not computable: the statistic's own value is below the smallest normal
        /// double, so it cannot hold its precision (decision 683).
        TooSmall => "too_small",
        /// Not computable: the predicted values' spread is at the level of their own
        /// rounding, so they cannot be standardized (decision 683).
        PredictedNoSpread => "predicted_no_spread",
        /// Note: the residual variance is below R's "essentially perfect fit"
        /// threshold, where the residual-scale numbers are rounding.
        EssentiallyPerfectFit => "essentially_perfect_fit",
        /// Excluded: the column's cell is empty or a missing-value token.
        MissingValue => "missing_value",
        /// Excluded: the column's cell is a value the user listed.
        MissingCode => "missing_code",
        /// Limitation: every predictor is entered at once; no stepwise methods
        /// and no blocks.
        EnterOnly => "enter_only",
        /// Limitation: each predictor is taken as a number; no indicator coding.
        NoCategoricalPredictors => "no_categorical_predictors",
        /// Limitation: no interaction or polynomial terms.
        NoInteractionTerms => "no_interaction_terms",
        /// Limitation: the model always has an intercept.
        InterceptAlways => "intercept_always",
        /// Limitation: listwise deletion only.
        ListwiseOnly => "listwise_only",
        /// Limitation: no heteroscedasticity-consistent standard errors.
        NoRobustStandardErrors => "no_robust_standard_errors",
        /// Limitation: no casewise diagnostics, plots or saved columns.
        NoCasewiseDiagnostics => "no_casewise_diagnostics",
        /// Limitation: the Durbin-Watson statistic takes the file's row order.
        DurbinWatsonRowOrder => "durbin_watson_row_order",
        /// Limitation: intervals at 95% only.
        Intervals95Only => "intervals_95_only",
        /// Limitation: each row counts once; no weights.
        NoWeights => "no_weights",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// The linear regression's parameters. `predictors` are in the user's order;
/// `missing_codes` apply by value to the outcome and every predictor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LinearRegressionParams {
    pub outcome: Alias,
    pub predictors: Vec<Alias>,
    pub missing_codes: Vec<f64>,
}

/* ------------------------------------------------------------------------ *
 *  The result
 * ------------------------------------------------------------------------ */

/// A number, or the closed reason it is not defined.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Value(f64),
    NotComputable(LinearRegressionCode),
}

/// One column of the model, the outcome first, over the complete rows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionColumn {
    pub column: Alias,
    pub mean: Stat,
    /// The standard deviation, n − 1.
    pub sd: Stat,
}

/// The model summary.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSummary {
    pub r: Stat,
    pub r_squared: Stat,
    pub adjusted_r_squared: Stat,
    /// σ, the root of the residual mean square.
    pub standard_error_of_estimate: Stat,
    pub durbin_watson: Stat,
}

/// A source of variation with its mean square.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnovaSource {
    pub sum_of_squares: Stat,
    pub df: u64,
    pub mean_square: Stat,
}

/// The total, which has no mean square.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnovaTotal {
    pub sum_of_squares: Stat,
    pub df: u64,
}

/// The analysis of variance of the regression.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionAnova {
    pub regression: AnovaSource,
    pub residual: AnovaSource,
    pub total: AnovaTotal,
    pub f: Stat,
    pub p: Stat,
}

/// The intercept's estimate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterceptEstimate {
    pub b: Stat,
    pub standard_error: Stat,
    pub t: Stat,
    pub p: Stat,
    pub lower: Stat,
    pub upper: Stat,
}

/// One predictor's estimate, in the list's order.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictorEstimate {
    pub column: Alias,
    pub b: Stat,
    pub standard_error: Stat,
    /// The standardized coefficient, b sd_x / sd_y.
    pub beta: Stat,
    pub t: Stat,
    pub p: Stat,
    pub lower: Stat,
    pub upper: Stat,
    pub zero_order: Stat,
    pub partial: Stat,
    pub part: Stat,
    pub tolerance: Stat,
    pub vif: Stat,
}

/// A variable's minimum, maximum, mean and standard deviation (n − 1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spread {
    pub minimum: Stat,
    pub maximum: Stat,
    pub mean: Stat,
    pub standard_deviation: Stat,
}

/// A variable's minimum and maximum.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Extremes {
    pub minimum: Stat,
    pub maximum: Stat,
}

/// The residuals' summary, as SPSS's Residuals Statistics table.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualSummary {
    pub predicted: Spread,
    pub residual: Spread,
    pub standardized_predicted: Extremes,
    pub standardized_residual: Extremes,
}

/// The linear regression's result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinearRegressionOutcome {
    pub outcome: Alias,
    pub rows_used: u64,
    /// The outcome first, then each predictor in the list's order.
    pub columns: Vec<RegressionColumn>,
    pub model: ModelSummary,
    pub anova: RegressionAnova,
    pub intercept: InterceptEstimate,
    /// In the list's order.
    pub predictors: Vec<PredictorEstimate>,
    pub residuals: ResidualSummary,
    /// The rows left out, by column and reason; none with no row is listed.
    pub exclusions: Vec<Exclusion<LinearRegressionCode>>,
    pub notes: Vec<LinearRegressionCode>,
    pub limitations: Vec<LinearRegressionCode>,
}

impl LinearRegressionOutcome {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, closed words and aliases serialises")
    }
}

/// What every result says it does not give, in decision 669's order.
const LIMITATIONS: [LinearRegressionCode; 10] = [
    LinearRegressionCode::EnterOnly,
    LinearRegressionCode::NoCategoricalPredictors,
    LinearRegressionCode::NoInteractionTerms,
    LinearRegressionCode::InterceptAlways,
    LinearRegressionCode::ListwiseOnly,
    LinearRegressionCode::NoRobustStandardErrors,
    LinearRegressionCode::NoCasewiseDiagnostics,
    LinearRegressionCode::DurbinWatsonRowOrder,
    LinearRegressionCode::Intervals95Only,
    LinearRegressionCode::NoWeights,
];

/* ------------------------------------------------------------------------ *
 *  The one reading of the rows
 * ------------------------------------------------------------------------ */

/// The complete rows' values, one vector per column, the outcome first and the
/// predictors in the list's order, and each column's rows left out. Never leaves
/// this module, and derives nothing.
struct Rows {
    values: Vec<Vec<f64>>,
    /// For each column: rows left out as `missing_value`, as `missing_code`.
    excluded: Vec<[u64; 2]>,
}

impl Rows {
    fn of(columns: &[&[Cell]], codes: &[f64]) -> Rows {
        let rows = columns.first().map_or(0, |c| c.len());
        let mut out = Rows {
            values: vec![Vec::with_capacity(rows); columns.len()],
            excluded: vec![[0; 2]; columns.len()],
        };
        let mut row = vec![0.0; columns.len()];
        'rows: for r in 0..rows {
            for (j, column) in columns.iter().enumerate() {
                match &column[r] {
                    Cell::Number(v) if codes.contains(v) => {
                        out.excluded[j][1] += 1;
                        continue 'rows;
                    }
                    Cell::Number(v) => row[j] = *v,
                    // Text is refused by validation before rows are read.
                    Cell::Missing | Cell::Text(_) => {
                        out.excluded[j][0] += 1;
                        continue 'rows;
                    }
                }
            }
            for (values, v) in out.values.iter_mut().zip(&row) {
                values.push(*v);
            }
        }
        out
    }

    fn complete(&self) -> usize {
        self.values.first().map_or(0, Vec::len)
    }
}

/// Every value equal as doubles, −0 equal to 0: Level 1's `constant`
/// (`stats_l1::compute`).
fn constant(xs: &[f64]) -> bool {
    xs.iter().all(|x| *x == xs[0])
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
    parameters: LinearRegressionParams,
) -> OperationSpec<LinearRegressionParams> {
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
        preparation: None,
    }
}

fn column_param(alias: Alias) -> ParamValue<LinearRegressionCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

/// The outcome, then the predictors in the list's order.
fn model_columns(params: &LinearRegressionParams) -> Vec<Alias> {
    std::iter::once(params.outcome).chain(params.predictors.iter().copied()).collect()
}

struct Checked<'d> {
    errors: Vec<Diagnostic<LinearRegressionCode>>,
    ready: Option<Vec<&'d [Cell]>>,
}

/// Shared by `validate` and `run` (decision 669's order): the missing codes, the
/// count of predictors, a column named twice, each column (the outcome first),
/// a code equal to a score's value; then, only when there is no other error, the
/// first of: no complete row, too few rows, the outcome constant, a predictor
/// constant.
fn check<'d>(dataset: &'d Dataset, params: &LinearRegressionParams) -> Checked<'d> {
    let mut errors = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(
                Diagnostic::error(LinearRegressionCode::MissingCodeNotFinite)
                    .int(ParamKey::Index, i as i64),
            );
        }
    }
    let count = params.predictors.len();
    if count == 0 {
        errors.push(Diagnostic::error(LinearRegressionCode::NoPredictors));
    } else if count > MAX_PREDICTORS {
        errors.push(
            Diagnostic::error(LinearRegressionCode::TooManyPredictors)
                .int(ParamKey::Count, count as i64)
                .int(ParamKey::Limit, MAX_PREDICTORS as i64),
        );
    }
    let all = model_columns(params);
    let mut seen = BTreeSet::new();
    for alias in &all {
        if !seen.insert(*alias) {
            errors.push(
                Diagnostic::error(LinearRegressionCode::ColumnRepeated)
                    .with(ParamKey::Column, column_param(*alias)),
            );
        }
    }
    let mut cells = Vec::with_capacity(all.len());
    for alias in &all {
        let Some((summary, column_cells)) = column(dataset, *alias) else {
            errors.push(
                Diagnostic::error(LinearRegressionCode::UnknownColumn)
                    .with(ParamKey::Column, column_param(*alias)),
            );
            continue;
        };
        let non_finite = non_finite_cells(summary);
        let texts = column_cells
            .iter()
            .filter(|c| matches!(c, Cell::Text(_)))
            .count() as i64;
        if non_finite > 0 {
            errors.push(
                Diagnostic::error(LinearRegressionCode::ColumnNonFinite)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, non_finite as i64),
            );
        } else if texts > 0 {
            errors.push(
                Diagnostic::error(LinearRegressionCode::ColumnHoldsText)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, texts),
            );
        }
        cells.push(column_cells);
    }
    let mut scanned = BTreeSet::new();
    for alias in all.iter().filter(|a| scanned.insert(**a)) {
        let collisions = score_code_count(dataset, *alias, &params.missing_codes);
        if collisions > 0 {
            errors.push(
                Diagnostic::error(LinearRegressionCode::CodeEqualsScore)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, collisions as i64),
            );
        }
    }
    if !errors.is_empty() {
        return Checked {
            errors,
            ready: None,
        };
    }
    let rows = Rows::of(&cells, &params.missing_codes);
    let n = rows.complete();
    let needed = count + 2;
    if n == 0 {
        errors.push(Diagnostic::error(LinearRegressionCode::NoCompleteRows));
    } else if n < needed {
        errors.push(
            Diagnostic::error(LinearRegressionCode::TooFewRows)
                .int(ParamKey::Count, n as i64)
                .int(ParamKey::Limit, needed as i64),
        );
    } else if constant(&rows.values[0]) {
        errors.push(Diagnostic::error(LinearRegressionCode::OutcomeConstant));
    } else if let Some(j) = rows.values[1..].iter().position(|x| constant(x)) {
        errors.push(
            Diagnostic::error(LinearRegressionCode::PredictorConstant)
                .with(ParamKey::Column, column_param(params.predictors[j])),
        );
    }
    Checked {
        errors,
        ready: Some(cells),
    }
}

/// Checks the spec against the dataset. Unsupported when it names another
/// capability, version or workspace; invalid on a wrong input or a parameter
/// error; a warning for each candidate missing code in a column that the user
/// did not list. Nothing needs input.
pub fn validate(
    dataset: &Dataset,
    spec: &OperationSpec<LinearRegressionParams>,
) -> ValidationReport<LinearRegressionCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID
    {
        unsupported.push(Diagnostic::error(LinearRegressionCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(LinearRegressionCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(LinearRegressionCode::InputMismatch));
            }
        }
        inputs => errors.push(
            Diagnostic::error(LinearRegressionCode::InputCount)
                .int(ParamKey::Count, inputs.len() as i64),
        ),
    }
    if !preparation_holds(dataset, spec.preparation.as_ref()) {
        errors.push(Diagnostic::error(LinearRegressionCode::PreparationMismatch));
    }
    let params = &spec.parameters;
    errors.extend(check(dataset, params).errors);
    let mut warnings = Vec::new();
    let mut warned = BTreeSet::new();
    for alias in &model_columns(params) {
        if !warned.insert(*alias) {
            continue;
        }
        if let Some((_, cells)) = column(dataset, *alias) {
            for s in suspect_codes_unless_score(dataset, *alias, cells) {
                if !params.missing_codes.contains(&f64::from(s.code)) {
                    warnings.push(
                        Diagnostic::warning(LinearRegressionCode::SuspectMissingCode)
                            .with(ParamKey::Column, column_param(*alias))
                            .int(ParamKey::Value, i64::from(s.code))
                            .int(ParamKey::Count, s.cells as i64),
                    );
                }
            }
        }
    }
    let rows = dataset.rows() as u64;
    let columns = (params.predictors.len() + 1) as u64;
    let estimate = ResourceEstimate {
        rows,
        columns,
        cells: rows.saturating_mul(columns),
    };
    ValidationReport::new(unsupported, errors, Vec::new(), warnings, estimate)
}

/* ------------------------------------------------------------------------ *
 *  run
 * ------------------------------------------------------------------------ */

/// Why [`run`] returned no result.
#[derive(Debug, Clone, PartialEq)]
pub enum Halt<B> {
    /// The parameters are wrong for this dataset, or the run's own refusal.
    Refused(Diagnostic<LinearRegressionCode>),
    /// The caller's check point stopped the run.
    Stopped { reason: B },
}

/// How many check points a run calls: one, before the arithmetic. The run
/// manager's progress counts it.
pub fn check_points() -> usize {
    1
}

/// [`run`] with a check point that never stops.
pub fn compute(
    dataset: &Dataset,
    params: &LinearRegressionParams,
) -> Result<LinearRegressionOutcome, Diagnostic<LinearRegressionCode>> {
    match run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The linear regression. `checkpoint(0)` is called after the parameters are
/// checked and before the arithmetic; a `Break` stops the run there. What a stop
/// means is the caller's.
pub fn run<B>(
    dataset: &Dataset,
    params: &LinearRegressionParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<LinearRegressionOutcome, Halt<B>> {
    let checked = check(dataset, params);
    if let Some(e) = checked.errors.into_iter().next() {
        return Err(Halt::Refused(e));
    }
    let cells = checked
        .ready
        .expect("no error means every column is good");
    if let ControlFlow::Break(reason) = checkpoint(0) {
        return Err(Halt::Stopped { reason });
    }
    let rows = Rows::of(&cells, &params.missing_codes);
    let fit = ols_qr_parts(&rows.values[0], &rows.values[1..]).map_err(|r| match r {
        QrRefusal::RankDeficient { predictor } if predictor >= 1 => Halt::Refused(
            Diagnostic::error(LinearRegressionCode::PredictorsCollinear)
                .with(ParamKey::Column, column_param(params.predictors[predictor - 1])),
        ),
        // The intercept, a column of ones, always passes the rank test, and the
        // checks leave more rows than parameters and columns of one length; a
        // double that cannot hold the fit is the only refusal left.
        _ => fault(),
    })?;
    model(params, &rows, &fit)
}

fn fault<B>() -> Halt<B> {
    Halt::Refused(Diagnostic::error(LinearRegressionCode::FitOutOfRange))
}

/// `v` as a value; an infinity is `overflow`, too large (decision 683). A NaN is
/// `too_small`, never `overflow`: FIND FIRST of 9A-2 found every NaN this module can
/// make with finite inputs to be a 0/0 of quantities that vanished, and the one it
/// reached, the standardized predicted values' with no spread, has its own code
/// before it gets here.
fn stat(v: f64) -> Stat {
    if v.is_finite() {
        Stat::Value(v)
    } else if v.is_nan() {
        TOO_SMALL
    } else {
        OVERFLOWED
    }
}

/// Below the smallest normal double and not 0: it holds only some of its digits.
fn subnormal(v: f64) -> bool {
    v != 0.0 && v.abs() < f64::MIN_POSITIVE
}

/// A statistic's own value: `too_small` when it is subnormal (decision 683),
/// otherwise as [`stat`].
fn own(v: f64) -> Stat {
    if subnormal(v) {
        TOO_SMALL
    } else {
        stat(v)
    }
}

const PERFECT: Stat = Stat::NotComputable(LinearRegressionCode::PerfectFit);
const OUT_OF_RANGE: Stat = Stat::NotComputable(LinearRegressionCode::DistributionOutOfRange);
const OVERFLOWED: Stat = Stat::NotComputable(LinearRegressionCode::Overflow);
const TOO_SMALL: Stat = Stat::NotComputable(LinearRegressionCode::TooSmall);
const NO_SPREAD: Stat = Stat::NotComputable(LinearRegressionCode::PredictedNoSpread);

/// `predicted_no_spread`'s constant c (decision 683), set by measurement in 9A-2:
/// over 19,800 datasets whose exact covariance with every predictor is 0 (gmp), the
/// largest S = s_ŷ / (ε (m + κ s_y)) was 3.635, so each gets the code; over 18,156
/// whose exact R² is 1e-12 to 1e-10 with |mean y| / sd y at most 1e8, the smallest
/// was 45.86, so none does (`~/gaply-corpus/os/phase9/9a2/nospread/`).
pub const NO_SPREAD_FACTOR: f64 = 8.0;

/// The sample variance (n − 1), refused as `fit_out_of_range` on a
/// `stats_verify` range fault.
fn variance<B>(xs: &[f64]) -> Result<f64, Halt<B>> {
    sample_variance(xs).map_err(|_| fault())
}

/// The standard deviation from its variance (n − 1). When the variance is
/// subnormal, the deviations from the mean are divided by the largest |deviation|
/// before they are squared, so the sum stays normal (decision 683); otherwise the
/// root of the variance, 9A-1's order.
fn sd_from<B>(xs: &[f64]) -> Result<f64, Halt<B>> {
    let v = variance(xs)?;
    if !subnormal(v) {
        return Ok(v.sqrt());
    }
    let m = mean(xs);
    let scale = xs.iter().fold(0.0f64, |a, x| a.max((x - m).abs()));
    let sum: f64 = xs.iter().map(|x| ((x - m) / scale).powi(2)).sum();
    Ok(scale * (sum / (xs.len() as f64 - 1.0)).sqrt())
}

/// A coefficient's standard error, t, p and interval, given σ², σ and the sum of
/// the squares of its row of R⁻¹ (s_j). In the normal range the SE is √(σ² s_j), as
/// `ols_qr` computes it; where σ² is subnormal, or σ² s_j is subnormal, 0 or
/// infinite, it is σ √s_j, whose intermediates stay normal (decision 683; the
/// review of d04199c4 found the split form gives h204's exact t). An s_j or an SE
/// that is itself subnormal is `too_small`; an infinite SE or t `overflow`.
fn estimate(b: f64, unscaled: f64, sigma2: f64, sigma: f64, df: f64, perfect: bool) -> [Stat; 5] {
    if perfect {
        return [PERFECT; 5];
    }
    // A NaN, 0 or subnormal s_j. An infinite s_j passes this test and is `overflow` below,
    // where σ √s_j is infinite; 9A-2's arm for it here could never run (the review of
    // 9e4b7ad9, finding 4; decision 698).
    if !(unscaled >= f64::MIN_POSITIVE) {
        return [TOO_SMALL; 5];
    }
    let product = sigma2 * unscaled;
    let se = if sigma2 >= f64::MIN_POSITIVE && product >= f64::MIN_POSITIVE && product.is_finite() {
        product.sqrt()
    } else {
        sigma * unscaled.sqrt()
    };
    if !se.is_finite() {
        return [OVERFLOWED; 5];
    }
    if !(se >= f64::MIN_POSITIVE) {
        return [TOO_SMALL; 5];
    }
    let [lower, upper] = match interval_95(b, se, df, LinearRegressionCode::DistributionOutOfRange, LinearRegressionCode::Overflow) {
        Ok([l, u]) => [stat(l), stat(u)],
        Err(c) => [Stat::NotComputable(c); 2],
    };
    let t = b / se;
    // An infinite t replaces even a finite interval with `overflow`. No data reach it: a
    // residual that is not 0 carries the fitted values' rounding, about ε times their size,
    // so t² is near MSS / RSS times df, at most about n / ε², far below 1.8e308; RSS of
    // exactly 0 is a perfect fit, coded above. Kept so the five stay one set if it is ever
    // reached (the review of 9e4b7ad9, finding 4; decision 698).
    if !t.is_finite() {
        return [Stat::Value(se), OVERFLOWED, OVERFLOWED, OVERFLOWED, OVERFLOWED];
    }
    if subnormal(t) {
        return [Stat::Value(se), TOO_SMALL, TOO_SMALL, lower, upper];
    }
    let p = match two_sided_p(t, df, LinearRegressionCode::DistributionOutOfRange, LinearRegressionCode::Overflow) {
        Ok(p) => stat(p),
        Err(c) => Stat::NotComputable(c),
    };
    [Stat::Value(se), Stat::Value(t), p, lower, upper]
}

/// The partial correlation t / √(t² + df), 9A-1's order; where t² overflows,
/// sign(t) / √(1 + df / t²), which is ±1 there (decision 683, the review's finding 7).
fn partial_of(t: f64, df: f64) -> f64 {
    let t2 = t * t;
    if t2.is_finite() {
        t / (t2 + df).sqrt()
    } else {
        t.signum() / (1.0 + df / t2).sqrt()
    }
}

/// The Durbin-Watson statistic, Σ (e_t − e_{t−1})² / RSS; where the numerator
/// overflows (RSS above about 4.5e307), from the residuals scaled by the largest
/// |residual| (decision 683), since the statistic is at most 4.
fn durbin_watson(residuals: &[f64], rss: f64) -> f64 {
    let numerator: f64 = residuals.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
    if numerator.is_finite() {
        return numerator / rss;
    }
    let s = residuals.iter().fold(0.0f64, |a, e| a.max(e.abs()));
    let num: f64 = residuals.windows(2).map(|w| ((w[1] - w[0]) / s).powi(2)).sum();
    let den: f64 = residuals.iter().map(|e| (e / s).powi(2)).sum();
    num / den
}

/// The minimum and maximum of finite values (`total_cmp`), as numbers.
fn extremes(xs: &[f64]) -> (f64, f64) {
    let min = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let max = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (min, max)
}

/// Whether the predicted values' spread is at the level of their own rounding
/// (decision 683): MSS exactly 0, or s_ŷ ≤ c ε (m + κ s_y), with s_ŷ the predicted
/// values' standard deviation, m the larger of |min ŷ| and |max ŷ|, s_y the outcome's
/// standard deviation, and κ the largest over the predictors of
/// √VIF_j √(1 + (n / (n − 1)) (mean_j / sd_j)²), VIF_j = 1 for one predictor, κ = 0
/// when a value it needs is a code. Computed from the result's own fields, in this
/// order, so the stored checker recomputes it exactly.
fn no_spread(
    mss: f64,
    predicted: &Spread,
    columns: &[RegressionColumn],
    predictors: &[PredictorEstimate],
    rows_used: u64,
) -> bool {
    if mss == 0.0 {
        return true;
    }
    let value = |s: Stat| match s {
        Stat::Value(v) => Some(v),
        Stat::NotComputable(_) => None,
    };
    let (Some(s_hat), Some(min), Some(max), Some(s_y)) = (
        value(predicted.standard_deviation),
        value(predicted.minimum),
        value(predicted.maximum),
        value(columns[0].sd),
    ) else {
        return false;
    };
    let m = min.abs().max(max.abs());
    let n = rows_used as f64;
    let one = predictors.len() == 1;
    let mut kappa = 0.0f64;
    for (j, p) in predictors.iter().enumerate() {
        let vif = if one { Some(1.0) } else { value(p.vif) };
        match (vif, value(columns[j + 1].mean), value(columns[j + 1].sd)) {
            (Some(vif), Some(mean_j), Some(sd_j)) => {
                let ratio = mean_j / sd_j;
                let g = vif.sqrt() * (1.0 + n / (n - 1.0) * (ratio * ratio)).sqrt();
                kappa = kappa.max(g);
            }
            _ => {
                kappa = 0.0;
                break;
            }
        }
    }
    s_hat <= NO_SPREAD_FACTOR * f64::EPSILON * (m + kappa * s_y)
}

/// Everything after the fit.
fn model<B>(
    params: &LinearRegressionParams,
    rows: &Rows,
    fit: &QrFit,
) -> Result<LinearRegressionOutcome, Halt<B>> {
    let y = &rows.values[0];
    let xs = &rows.values[1..];
    let n = rows.complete();
    let k = params.predictors.len();
    let df = fit.df_residual;
    let (rss, mss) = (fit.rss, fit.mss);
    let perfect = rss == 0.0;
    let sigma2 = rss / df;
    // σ: √σ² in the normal range, 9A-1's order; √RSS / √df when σ² is subnormal,
    // whose terms stay normal (decision 683). RSS is 0 or normal (`sum_of_squares`).
    let sigma = if sigma2 >= f64::MIN_POSITIVE { sigma2.sqrt() } else { rss.sqrt() / df.sqrt() };
    let unscaled = |i: usize| fit.r_inv[i][i..].iter().map(|x| x * x).sum::<f64>();

    // Each column's mean and standard deviation; the variance first, so a mean
    // a double cannot hold is its range fault (stats_verify::sample_variance).
    let mut variances = Vec::with_capacity(k + 1);
    let mut sds = Vec::with_capacity(k + 1);
    let mut columns = Vec::with_capacity(k + 1);
    for (alias, values) in model_columns(params).into_iter().zip(&rows.values) {
        let v = variance(values)?;
        let s = sd_from(values)?;
        variances.push(v);
        sds.push(s);
        columns.push(RegressionColumn {
            column: alias,
            mean: stat(mean(values)),
            sd: stat(s),
        });
    }

    let r_squared = mss / (mss + rss);
    let adjusted = 1.0 - (1.0 - r_squared) * (n as f64 - 1.0) / df;
    let fitted: Vec<f64> = y.iter().zip(&fit.residuals).map(|(y, e)| y - e).collect();
    let model = ModelSummary {
        r: stat(r_squared.sqrt()),
        r_squared: stat(r_squared),
        adjusted_r_squared: stat(adjusted),
        standard_error_of_estimate: if perfect { PERFECT } else { stat(sigma) },
        durbin_watson: if perfect { PERFECT } else { stat(durbin_watson(&fit.residuals, rss)) },
    };

    let regression_ms = mss / k as f64;
    let (f, f_p) = if perfect {
        (PERFECT, PERFECT)
    } else {
        // 9A-1's order in the normal range; (MSS / RSS)(df / k) where σ² or MSS / k
        // is subnormal (decision 683).
        let f = if sigma2 >= f64::MIN_POSITIVE && !subnormal(regression_ms) {
            regression_ms / sigma2
        } else {
            (mss / rss) * (df / k as f64)
        };
        if !f.is_finite() {
            (OVERFLOWED, OVERFLOWED)
        } else if subnormal(f) {
            (TOO_SMALL, TOO_SMALL)
        } else {
            (Stat::Value(f), f_upper(f, k as f64, df).map_or(OUT_OF_RANGE, stat))
        }
    };
    let anova = RegressionAnova {
        regression: AnovaSource {
            sum_of_squares: stat(mss),
            df: k as u64,
            mean_square: own(regression_ms),
        },
        residual: AnovaSource {
            sum_of_squares: stat(rss),
            df: df as u64,
            mean_square: if perfect { PERFECT } else { own(sigma2) },
        },
        total: AnovaTotal {
            sum_of_squares: stat(mss + rss),
            df: (n - 1) as u64,
        },
        f,
        p: f_p,
    };

    let [se, t, p, lower, upper] = estimate(fit.coefficients[0], unscaled(0), sigma2, sigma, df, perfect);
    let intercept = InterceptEstimate {
        b: stat(fit.coefficients[0]),
        standard_error: se,
        t,
        p,
        lower,
        upper,
    };

    let sd_y = sds[0];
    let unexplained = rss / (mss + rss);
    let mut predictors = Vec::with_capacity(k);
    for (j, alias) in params.predictors.iter().enumerate() {
        let i = j + 1;
        let b = fit.coefficients[i];
        let s_j = unscaled(i);
        let [se, t, p, lower, upper] = estimate(b, s_j, sigma2, sigma, df, perfect);
        let zero_order = pearson_clamped(&xs[j], y).map_err(|_| fault())?.coefficient;
        let (partial, part) = match t {
            _ if perfect => (PERFECT, PERFECT),
            Stat::Value(t) => (stat(partial_of(t, df)), stat(t * (unexplained / df).sqrt())),
            other => (other, other),
        };
        let (tolerance, vif) = if k == 1 {
            // With one predictor there is nothing to be collinear with: both
            // exactly 1, as SPSS prints them, never the arithmetic's rounding.
            (Stat::Value(1.0), Stat::Value(1.0))
        } else if !(s_j >= f64::MIN_POSITIVE) {
            // An s_j that is subnormal has lost its digits (decision 683).
            (TOO_SMALL, TOO_SMALL)
        } else {
            // (n − 1) var_j s_j, 9A-1's order; where var_j is subnormal, from the
            // standard deviation so computed, (sd_j √s_j)² (n − 1) (decision 683).
            let vif = if subnormal(variances[i]) {
                let g = sds[i] * s_j.sqrt();
                g * g * (n as f64 - 1.0)
            } else {
                let ss = (n as f64 - 1.0) * variances[i];
                ss * s_j
            };
            (stat(1.0 / vif), stat(vif))
        };
        predictors.push(PredictorEstimate {
            column: *alias,
            b: stat(b),
            standard_error: se,
            beta: own(b * sds[i] / sd_y),
            t,
            p,
            lower,
            upper,
            zero_order: stat(zero_order),
            partial,
            part,
            tolerance,
            vif,
        });
    }

    let residuals = residual_summary(&fitted, &fit.residuals, sigma, perfect, mss, &columns, &predictors, n as u64)?;

    let mut notes = Vec::new();
    let fitted_variance = sample_variance(&fitted).map_err(|_| fault())?;
    let fitted_mean = mean(&fitted);
    if sigma2.is_finite()
        && sigma2 < (fitted_mean * fitted_mean + fitted_variance) * ESSENTIALLY_PERFECT_FACTOR
    {
        notes.push(LinearRegressionCode::EssentiallyPerfectFit);
    }

    let mut exclusions = Vec::new();
    for (alias, counts) in model_columns(params).iter().zip(&rows.excluded) {
        for (code, count) in [
            (LinearRegressionCode::MissingValue, counts[0]),
            (LinearRegressionCode::MissingCode, counts[1]),
        ] {
            if count > 0 {
                exclusions.push(Exclusion {
                    code,
                    count,
                    params: BTreeMap::from([(ParamKey::Column, column_param(*alias))]),
                });
            }
        }
    }
    Ok(LinearRegressionOutcome {
        outcome: params.outcome,
        rows_used: n as u64,
        columns,
        model,
        anova,
        intercept,
        predictors,
        residuals,
        exclusions,
        notes,
        limitations: LIMITATIONS.to_vec(),
    })
}

/// The residuals' summary, as SPSS's Residuals Statistics table.
#[allow(clippy::too_many_arguments)]
fn residual_summary<B>(
    fitted: &[f64],
    residuals: &[f64],
    sigma: f64,
    perfect: bool,
    mss: f64,
    columns: &[RegressionColumn],
    predictors: &[PredictorEstimate],
    rows_used: u64,
) -> Result<ResidualSummary, Halt<B>> {
    let spread = |xs: &[f64]| -> Result<(Spread, f64, f64), Halt<B>> {
        let s = sd_from(xs)?;
        let m = mean(xs);
        let (min, max) = extremes(xs);
        Ok((
            Spread {
                minimum: stat(min),
                maximum: stat(max),
                mean: stat(m),
                standard_deviation: stat(s),
            },
            m,
            s,
        ))
    };
    let (predicted, f_mean, f_sd) = spread(fitted)?;
    let (residual, _, _) = spread(residuals)?;
    let (f_min, f_max) = extremes(fitted);
    let standardized_predicted = if no_spread(mss, &predicted, columns, predictors, rows_used) {
        Extremes {
            minimum: NO_SPREAD,
            maximum: NO_SPREAD,
        }
    } else {
        Extremes {
            minimum: stat((f_min - f_mean) / f_sd),
            maximum: stat((f_max - f_mean) / f_sd),
        }
    };
    let standardized_residual = if perfect {
        Extremes {
            minimum: PERFECT,
            maximum: PERFECT,
        }
    } else {
        let (e_min, e_max) = extremes(residuals);
        Extremes {
            minimum: stat(e_min / sigma),
            maximum: stat(e_max / sigma),
        }
    };
    Ok(ResidualSummary {
        predicted,
        residual,
        standardized_predicted,
        standardized_residual,
    })
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows used
/// are the complete rows; with the exclusions they add up to the data rows. The
/// output hash is SHA-256 of [`LinearRegressionOutcome::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<LinearRegressionParams>,
    dataset: &Dataset,
    result: &LinearRegressionOutcome,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<LinearRegressionCode> {
    let parameters =
        serde_json::to_vec(&spec.parameters).expect("parameters of aliases and numbers serialise");
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        inputs: spec.inputs.iter().map(|i| i.content_hash.clone()).collect(),
        parameters_hash: ContentHash::of_bytes(&parameters),
        seed: spec.random_seed,
        rows_read: dataset.rows() as u64,
        rows_used: result.rows_used,
        exclusions: result.exclusions.clone(),
        platform,
        elapsed_ms,
        output_hash: ContentHash::of_bytes(&result.canonical_bytes()),
        reproducibility: Reproducibility::Bitwise,
        preparation: spec.preparation.clone(),
    }
}

/// What no dataset reaches, with crafted numbers.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_perfect_fit_is_its_own_code_and_an_se_that_overflows_or_vanishes_has_its_own() {
        assert_eq!(estimate(1.0, 1.0, 0.0, 0.0, 3.0, true), [PERFECT; 5]);
        // σ 0 off a perfect fit (no input reaches it): the SE is 0, too small.
        assert_eq!(estimate(1.0, 1.0, 0.0, 0.0, 3.0, false), [TOO_SMALL; 5]);
        assert_eq!(estimate(1.0, f64::INFINITY, 1.0, 1.0, 3.0, false), [OVERFLOWED; 5]);
        // A normal SE of 1e-10 and a t of 1e310.
        let [se, t, ..] = estimate(1e300, 1e-10, 1e-10, 1e-5, 3.0, false);
        assert!(matches!(se, Stat::Value(_)), "{se:?}");
        assert_eq!(t, OVERFLOWED);
    }

    #[test]
    fn a_subnormal_product_takes_the_split_form_and_a_subnormal_s_j_is_too_small() {
        // σ² 1e-145, s_j 1e-170: the product is subnormal, σ √s_j = 1e-72.5 · 1e-85 is normal.
        let sigma2 = 1e-145;
        let [se, ..] = estimate(1.0, 1e-170, sigma2, sigma2.sqrt(), 3.0, false);
        let want = sigma2.sqrt() * 1e-170f64.sqrt();
        assert_eq!(se, Stat::Value(want));
        assert_eq!(estimate(1.0, 1e-310, 1e10, 1e5, 3.0, false), [TOO_SMALL; 5]);
        // In the normal range the SE is √(σ² s_j), bit for bit.
        let [se, ..] = estimate(1.0, 1e-150, 1e-120, 1e-60, 3.0, false);
        assert_eq!(se, Stat::Value((1e-120f64 * 1e-150).sqrt()));
    }

    #[test]
    fn a_df_past_the_measured_range_gives_the_distribution_code() {
        let [_, _, p, lower, upper] = estimate(1.0, 1.0, 1.0, 1.0, 2e7, false);
        assert_eq!((p, lower, upper), (OUT_OF_RANGE, OUT_OF_RANGE, OUT_OF_RANGE));
    }

    /// Decision 683 (the review's finding 7): t² overflows above |t| 1.34e154, where
    /// 9A-1's t / √(t² + df) gave 0; the overflow form gives ±1, and below it the
    /// value is 9A-1's, bit for bit.
    #[test]
    fn partial_is_one_in_size_where_t_squared_overflows() {
        assert_eq!(partial_of(1e200, 5.0), 1.0);
        assert_eq!(partial_of(-1e200, 5.0), -1.0);
        assert_eq!(partial_of(3.0, 5.0), 3.0 / (9.0f64 + 5.0).sqrt());
    }

    /// Decision 683: the numerator overflows while the statistic is at most 4.
    #[test]
    fn the_durbin_watson_statistic_is_computed_when_its_numerator_overflows() {
        let e = [6e153, -6e153, 6e153, -6e153];
        let rss: f64 = e.iter().map(|x| x * x).sum();
        assert!(rss.is_finite() && e.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f64>().is_infinite(), "the premise");
        assert_eq!(durbin_watson(&e, rss), 3.0);
        let small = [1.0, -1.0, 2.0];
        assert_eq!(durbin_watson(&small, 6.0), (4.0 + 9.0) / 6.0);
    }

    /// Decision 683: a NaN is never `overflow`; an infinity is.
    #[test]
    fn stat_keeps_overflow_for_too_large() {
        assert_eq!(stat(f64::INFINITY), OVERFLOWED);
        assert_eq!(stat(f64::NAN), TOO_SMALL);
        assert_eq!(own(1e-310), TOO_SMALL);
        assert_eq!(own(0.0), Stat::Value(0.0));
    }

    #[test]
    fn constant_takes_minus_zero_as_zero() {
        assert!(constant(&[0.0, -0.0, 0.0]));
        assert!(!constant(&[0.0, 1e-300]));
    }
}
