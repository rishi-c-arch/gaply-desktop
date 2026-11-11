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
//! `distribution_out_of_range`; any other value a double cannot hold is
//! `overflow`, statistic by statistic. The run is refused only by its checks and
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
        /// Not computable: a sum, product, quotient or root a double cannot hold.
        Overflow => "overflow",
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

/// `v`, or not computable with `overflow` when it is infinite or NaN.
fn stat(v: f64) -> Stat {
    if v.is_finite() {
        Stat::Value(v)
    } else {
        Stat::NotComputable(LinearRegressionCode::Overflow)
    }
}

const PERFECT: Stat = Stat::NotComputable(LinearRegressionCode::PerfectFit);
const OUT_OF_RANGE: Stat = Stat::NotComputable(LinearRegressionCode::DistributionOutOfRange);
const OVERFLOWED: Stat = Stat::NotComputable(LinearRegressionCode::Overflow);

/// The sample variance (n − 1), refused as `fit_out_of_range` on a
/// `stats_verify` range fault.
fn variance<B>(xs: &[f64]) -> Result<f64, Halt<B>> {
    sample_variance(xs).map_err(|_| fault())
}

/// Whether a variance holds its digits: 0, or a normal double. A subnormal one
/// keeps only a few significant bits, so nothing is built on it (the review of
/// 9A-1, after the hunt's h204).
fn held(variance: f64) -> bool {
    variance == 0.0 || variance >= f64::MIN_POSITIVE
}

/// The standard deviation from a variance, `overflow` when the variance does not
/// hold its digits.
fn sd_of(variance: f64) -> Stat {
    if held(variance) {
        stat(variance.sqrt())
    } else {
        OVERFLOWED
    }
}

/// A coefficient's standard error, t, p and interval, given the residual
/// variance and the sum of the squares of its row of R⁻¹.
fn estimate(b: f64, unscaled: f64, sigma2: f64, df: f64, perfect: bool) -> [Stat; 5] {
    if perfect {
        return [PERFECT; 5];
    }
    // As `ols_qr` computes it, √(σ² s_j). A product, or an s_j, below the
    // smallest normal double keeps only a few significant bits, so its root is a
    // finite, wrong SE (the hunt's h204: R's lm printed t −8.70 where the exact t
    // is −12.31). A double cannot hold it: `overflow`, as for an infinite one.
    let variance = sigma2 * unscaled;
    if !(sigma2 >= f64::MIN_POSITIVE)
        || !(unscaled >= f64::MIN_POSITIVE)
        || !(variance >= f64::MIN_POSITIVE)
        || !variance.is_finite()
    {
        return [OVERFLOWED; 5];
    }
    let se = variance.sqrt();
    let t = b / se;
    if !t.is_finite() {
        return [Stat::Value(se), OVERFLOWED, OVERFLOWED, OVERFLOWED, OVERFLOWED];
    }
    let p = match two_sided_p(t, df, LinearRegressionCode::DistributionOutOfRange, LinearRegressionCode::Overflow) {
        Ok(p) => stat(p),
        Err(c) => Stat::NotComputable(c),
    };
    let [lower, upper] = match interval_95(b, se, df, LinearRegressionCode::DistributionOutOfRange, LinearRegressionCode::Overflow) {
        Ok([l, u]) => [stat(l), stat(u)],
        Err(c) => [Stat::NotComputable(c); 2],
    };
    [Stat::Value(se), Stat::Value(t), p, lower, upper]
}

/// The minimum and maximum of finite values (`total_cmp`), as numbers.
fn extremes(xs: &[f64]) -> (f64, f64) {
    let min = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let max = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (min, max)
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
    // σ² must hold its digits for anything built on it (the review of 9A-1).
    let sigma_held = sigma2 >= f64::MIN_POSITIVE;
    let unscaled = |i: usize| fit.r_inv[i][i..].iter().map(|x| x * x).sum::<f64>();

    // Each column's mean and standard deviation; the variance first, so a mean
    // a double cannot hold is its range fault (stats_verify::sample_variance).
    let mut variances = Vec::with_capacity(k + 1);
    let mut sds = Vec::with_capacity(k + 1);
    let mut columns = Vec::with_capacity(k + 1);
    for (alias, values) in model_columns(params).into_iter().zip(&rows.values) {
        let v = variance(values)?;
        let s = v.sqrt();
        variances.push(v);
        sds.push(s);
        columns.push(RegressionColumn {
            column: alias,
            mean: stat(mean(values)),
            sd: sd_of(v),
        });
    }

    let r_squared = mss / (mss + rss);
    let adjusted = 1.0 - (1.0 - r_squared) * (n as f64 - 1.0) / df;
    let fitted: Vec<f64> = y.iter().zip(&fit.residuals).map(|(y, e)| y - e).collect();
    let durbin_watson = if perfect {
        PERFECT
    } else {
        let numerator: f64 = fit.residuals.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
        stat(numerator / rss)
    };
    let model = ModelSummary {
        r: stat(r_squared.sqrt()),
        r_squared: stat(r_squared),
        adjusted_r_squared: stat(adjusted),
        standard_error_of_estimate: if perfect {
            PERFECT
        } else if sigma_held {
            stat(sigma2.sqrt())
        } else {
            OVERFLOWED
        },
        durbin_watson,
    };

    let regression_ms = mss / k as f64;
    let (f, f_p) = if perfect {
        (PERFECT, PERFECT)
    } else if !sigma_held {
        (OVERFLOWED, OVERFLOWED)
    } else {
        let f = regression_ms / sigma2;
        if f.is_finite() {
            (Stat::Value(f), f_upper(f, k as f64, df).map_or(OUT_OF_RANGE, stat))
        } else {
            (OVERFLOWED, OVERFLOWED)
        }
    };
    let anova = RegressionAnova {
        regression: AnovaSource {
            sum_of_squares: stat(mss),
            df: k as u64,
            mean_square: stat(regression_ms),
        },
        residual: AnovaSource {
            sum_of_squares: stat(rss),
            df: df as u64,
            mean_square: if perfect {
                PERFECT
            } else if sigma_held {
                stat(sigma2)
            } else {
                OVERFLOWED
            },
        },
        total: AnovaTotal {
            sum_of_squares: stat(mss + rss),
            df: (n - 1) as u64,
        },
        f,
        p: f_p,
    };

    let [se, t, p, lower, upper] = estimate(fit.coefficients[0], unscaled(0), sigma2, df, perfect);
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
        let [se, t, p, lower, upper] = estimate(b, s_j, sigma2, df, perfect);
        let zero_order = pearson_clamped(&xs[j], y).map_err(|_| fault())?.coefficient;
        let (partial, part) = match t {
            _ if perfect => (PERFECT, PERFECT),
            Stat::Value(t) => (stat(t / (t * t + df).sqrt()), stat(t * (unexplained / df).sqrt())),
            other => (other, other),
        };
        let (tolerance, vif) = if k == 1 {
            // With one predictor there is nothing to be collinear with: both
            // exactly 1, as SPSS prints them, never the arithmetic's rounding.
            (Stat::Value(1.0), Stat::Value(1.0))
        } else if !(s_j >= f64::MIN_POSITIVE) || !held(variances[i]) {
            // An s_j below the smallest normal double has lost its digits, as
            // in `estimate`.
            (OVERFLOWED, OVERFLOWED)
        } else {
            let ss = (n as f64 - 1.0) * variances[i];
            let vif = ss * s_j;
            (stat(1.0 / vif), stat(vif))
        };
        predictors.push(PredictorEstimate {
            column: *alias,
            b: stat(b),
            standard_error: se,
            beta: if held(variances[i]) && held(variances[0]) {
                stat(b * sds[i] / sd_y)
            } else {
                OVERFLOWED
            },
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

    let residuals = residual_summary(&fitted, &fit.residuals, sigma2, perfect, sigma_held)?;

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
fn residual_summary<B>(
    fitted: &[f64],
    residuals: &[f64],
    sigma2: f64,
    perfect: bool,
    sigma_held: bool,
) -> Result<ResidualSummary, Halt<B>> {
    let spread = |xs: &[f64]| -> Result<(Spread, f64, f64, bool), Halt<B>> {
        let v = variance(xs)?;
        let s = v.sqrt();
        let m = mean(xs);
        let (min, max) = extremes(xs);
        Ok((
            Spread {
                minimum: stat(min),
                maximum: stat(max),
                mean: stat(m),
                standard_deviation: sd_of(v),
            },
            m,
            s,
            held(v),
        ))
    };
    let (predicted, f_mean, f_sd, f_held) = spread(fitted)?;
    let (residual, _, _, _) = spread(residuals)?;
    let (f_min, f_max) = extremes(fitted);
    let standardized_predicted = if f_held {
        Extremes {
            minimum: stat((f_min - f_mean) / f_sd),
            maximum: stat((f_max - f_mean) / f_sd),
        }
    } else {
        Extremes {
            minimum: OVERFLOWED,
            maximum: OVERFLOWED,
        }
    };
    let standardized_residual = if perfect {
        Extremes {
            minimum: PERFECT,
            maximum: PERFECT,
        }
    } else if !sigma_held {
        Extremes {
            minimum: OVERFLOWED,
            maximum: OVERFLOWED,
        }
    } else {
        let sigma = sigma2.sqrt();
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
    fn a_coefficient_with_no_spread_left_in_its_se_is_overflow_and_a_perfect_fit_is_its_own_code() {
        assert_eq!(estimate(1.0, 1.0, 0.0, 3.0, true), [PERFECT; 5]);
        // σ² underflowing to 0 with RSS above 0: the SE is 0 and t would be infinite.
        assert_eq!(estimate(1.0, 1.0, 0.0, 3.0, false), [OVERFLOWED; 5]);
        assert_eq!(estimate(1.0, f64::INFINITY, 1.0, 3.0, false), [OVERFLOWED; 5]);
        // A normal SE of 1e-10 and a t of 1e310.
        let [se, t, ..] = estimate(1e300, 1e-10, 1e-10, 3.0, false);
        assert!(matches!(se, Stat::Value(_)), "{se:?}");
        assert_eq!(t, OVERFLOWED);
    }

    #[test]
    fn a_subnormal_variance_or_s_j_is_overflow() {
        assert_eq!(estimate(1.0, 1e-170, 1e-145, 3.0, false), [OVERFLOWED; 5]);
        assert_eq!(estimate(1.0, 1e-310, 1e10, 3.0, false), [OVERFLOWED; 5]);
        let [se, ..] = estimate(1.0, 1e-150, 1e-120, 3.0, false);
        assert!(matches!(se, Stat::Value(_)), "{se:?}");
    }

    #[test]
    fn a_df_past_the_measured_range_gives_the_distribution_code() {
        let [_, _, p, lower, upper] = estimate(1.0, 1.0, 1.0, 2e7, false);
        assert_eq!((p, lower, upper), (OUT_OF_RANGE, OUT_OF_RANGE, OUT_OF_RANGE));
    }

    #[test]
    fn constant_takes_minus_zero_as_zero() {
        assert!(constant(&[0.0, -0.0, 0.0]));
        assert!(!constant(&[0.0, 1e-300]));
    }
}
