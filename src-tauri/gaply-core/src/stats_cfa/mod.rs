//! **`gaply_core::stats_cfa`: confirmatory factor analysis on the user's data,
//! as AMOS fits a measurement model (Research OS, Phase 9C-1, decisions 743 to
//! 746).** Each factor is measured by two or more numeric columns, each column
//! by one factor; the first column of each factor has its loading fixed at 1, as
//! AMOS draws a model; the error variances and the factor variances are free,
//! and, when asked, the factor covariances. No means are estimated.
//!
//! **AMOS's conventions (decision 743).** The model is fitted to S, the sample
//! covariance matrix with divisor N (the maximum likelihood estimate), by
//! minimising F(θ) = ln|Σ(θ)| + tr(S Σ(θ)⁻¹) − ln|S| − p; CMIN is (N − 1) · F
//! at its minimum; the standard errors are the square roots of the diagonal of
//! [(N − 1) Δᵀ W Δ]⁻¹, the expected information at the fitted Σ̂, which is
//! [(N − 1) H / 2]⁻¹ with H below; C.R. is the estimate over its standard
//! error and its P two-tailed standard normal. The fit measures are AMOS's own
//! (its Appendix C), built from CMIN.
//!
//! **The estimation (decision 745).** Fisher scoring from the starts φ_ff =
//! s_mm / 2 (m the factor's first column), λ_i = s_im / φ_ff, θ_ii = s_ii / 2
//! and, when correlated, φ_fg = s_{m_f m_g} / 2. Each step is δ = −H⁻¹ g, with g_a
//! = tr(Σ⁻¹ (Σ − S) Σ⁻¹ Σ_a) and H_ab = tr(Σ⁻¹ Σ_a Σ⁻¹ Σ_b): each Σ_a is one or two
//! outer products, so each entry is a product of two bilinear forms in Σ⁻¹, Σ⁻¹Λ
//! and ΛᵀΣ⁻¹Λ, and no duplication matrix is formed. The step is halved until Σ
//! is positive definite and, while the Newton decrement gᵀH⁻¹g is above 1e-10,
//! F does not increase; the run stops when the decrement is at most 1e-20,
//! which does not depend on the data's units. H failing its Cholesky factor
//! (the 1e-9 rule) at any iteration is a model that is not identified.
//!
//! **Not computable, statistic by statistic**, as a closed code beside the
//! value it replaces: `fixed_parameter`, `df_zero`, `cmin_zero`,
//! `too_few_rows_for_bcc`, `negative_variance`, `overflow`, `too_small` and
//! `distribution_out_of_range`. Notes are bare codes; no word says a fit is
//! good, poor, close, acceptable or adequate, and no cutoff is named.
//!
//! **What never leaves.** Factors are numbered 1 to k; no name a user typed is
//! in the spec, the result or a line written anywhere (ADR-011). The result
//! holds no cell, no header and no p × p matrix: numbers, closed words and
//! aliases.
//!
//! Pure: no file, network, process, clock, model or log
//! (`tests/contracts_are_pure.rs`). lavaan 0.7-3, refined by its own Fisher
//! scoring in R, is the oracle (`tests/fixtures/stats_cfa/make_cfa_fixtures.R`,
//! compared in `tests/cfa_r_golden.rs`); AMOS's printed Example 8 is the second
//! (`tests/cfa_amos.rs`). Nothing of lavaan's or AMOS's code is in Gaply.

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
use crate::stats_dist::{
    chi_square_quantile_upper, chi_square_upper_wide, noncentral_chi_square_ncp,
    noncentral_chi_square_tails, normal_two_tailed,
};
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_linalg::{cholesky, cholesky_within, log_det, multiply, spd_inverse};
use crate::stats_ttest::{column, non_finite_cells};

pub mod export;

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.cfa";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;

/// The fewest columns in a model.
pub const MIN_ITEMS: usize = 3;
/// The most columns in a model (decision 745).
pub const MAX_ITEMS: usize = 128;
/// The most factors (decision 745).
pub const MAX_FACTORS: usize = 16;
/// The fewest columns measuring one factor.
pub const MIN_FACTOR_ITEMS: usize = 2;
/// The iterations before the run refuses as `cfa_not_converged` (decision 745).
pub const MAX_ITERATIONS: u32 = 500;
/// The run stops when the Newton decrement gᵀH⁻¹g is at most this (decision
/// 745): far tighter than AMOS's own rule, so printed values agree.
pub const DECREMENT_STOP: f64 = 1e-20;
/// Above this decrement a step must not increase F; at or below it F's change
/// is below its own rounding, and only Σ's positive definiteness is asked
/// (decision 745).
pub const DECREMENT_F_TEST: f64 = 1e-10;
/// The halvings of a step before the run refuses (decision 745).
pub const MAX_HALVINGS: u32 = 30;
/// PCLOSE's RMSEA under the null, 0.05: its ncp is 0.05² n d (AMOS, Appendix C).
pub const PCLOSE_RMSEA: f64 = 0.05;

closed! {
    /// Every refusal, outcome, exclusion, note and limitation of the
    /// confirmatory factor analysis.
    pub enum CfaCode in "stats.cfa." {
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
        /// A missing code is listed twice. Param `index`.
        MissingCodeRepeated => "missing_code_repeated",
        /// The missing codes are not ascending. Param `index`.
        MissingCodesOutOfOrder => "missing_codes_out_of_order",
        /// The model names no factor.
        NoFactors => "no_factors",
        /// More factors than the limit. Params `count`, `limit`.
        TooManyFactors => "too_many_factors",
        /// A factor is measured by fewer than two columns. Params `index` (the
        /// factor's number), `count`, `limit`.
        FactorTooFewItems => "factor_too_few_items",
        /// Fewer columns in all than the limit. Params `count`, `limit`.
        TooFewItems => "too_few_items",
        /// More columns in all than the limit. Params `count`, `limit`.
        TooManyItems => "too_many_items",
        /// A factor's columns are not in the dataset's order. Param `column`.
        ItemsOutOfOrder => "items_out_of_order",
        /// The factors are not in the order of their first columns. Param
        /// `index` (the factor's number).
        FactorsOutOfOrder => "factors_out_of_order",
        /// A column is named twice, within one factor or across two. Param
        /// `column`.
        ItemRepeated => "item_repeated",
        /// A column the dataset does not have. Param `column`.
        UnknownColumn => "unknown_column",
        /// A column holds NaN or an infinity. Params `column`, `count`.
        ColumnNonFinite => "column_non_finite",
        /// A column holds text. Params `column`, `count`.
        ColumnHoldsText => "column_holds_text",
        /// A missing code equals a scale score's value in the column. Params
        /// `column`, `count`.
        CodeEqualsScore => "code_equals_score",
        /// No row has a value in every column.
        NoCompleteRows => "no_complete_rows",
        /// Fewer complete rows than columns plus one, so S cannot be positive
        /// definite. Params `count`, `limit`.
        TooFewRows => "too_few_rows",
        /// A column takes one value over the complete rows. Param `column`.
        ItemConstant => "item_constant",
        /// The run's refusal: S's Cholesky factor refuses at this column, which
        /// is a combination of the columns before it within the arithmetic.
        /// Param `column`.
        SampleCovarianceSingular => "sample_covariance_singular",
        /// The run's refusal: the information matrix fails its Cholesky factor,
        /// so the model is not identified; with param `count`, the model has
        /// that many more parameters than moments.
        ModelNotIdentified => "model_not_identified",
        /// The run's refusal: the estimation did not reach its stop within the
        /// iterations, or no step kept Σ positive definite. Param `limit`.
        CfaNotConverged => "cfa_not_converged",
        /// A candidate missing code the rule found in a column and the user did
        /// not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Not computable: a fixed loading has no standard error, C.R. or P.
        FixedParameter => "fixed_parameter",
        /// Not computable: the model has 0 degrees of freedom. Also a note.
        DfZero => "df_zero",
        /// Not computable: HOELTER at a CMIN of 0.
        CminZero => "cmin_zero",
        /// Not computable: BCC and MECVI need more than p + 2 rows.
        TooFewRowsForBcc => "too_few_rows_for_bcc",
        /// Not computable: a square root meets a variance at or below 0. Also
        /// a note: a variance estimate below 0.
        NegativeVariance => "negative_variance",
        /// Not computable: a 0/0 of quantities that vanished (a NaN). A value
        /// below the smallest normal double is given as a value (the hunt saw a
        /// P of 4.4e-323).
        TooSmall => "too_small",
        /// Not computable: a value along the way too large for a double.
        Overflow => "overflow",
        /// Not computable: the distribution is not measured here.
        DistributionOutOfRange => "distribution_out_of_range",
        /// Note: the factors' covariance matrix at the solution is not positive
        /// definite.
        FactorCovarianceNotPositiveDefinite => "factor_covariance_not_positive_definite",
        /// Excluded: a column's cell is empty or a missing-value token.
        MissingValue => "missing_value",
        /// Excluded: a column's cell is a value the user listed.
        MissingCode => "missing_code",
        /// Limitation: maximum likelihood is the only estimator.
        MlOnly => "ml_only",
        /// Limitation: no means or intercepts.
        NoMeans => "no_means",
        /// Limitation: one group.
        OneGroupOnly => "one_group_only",
        /// Limitation: each column measures one factor; no cross-loadings or
        /// correlated errors.
        SimpleStructureOnly => "simple_structure_only",
        /// Limitation: each factor's scale is its first column's loading of 1.
        MarkerIdentificationOnly => "marker_identification_only",
        /// Limitation: no modification indices.
        NoModificationIndices => "no_modification_indices",
        /// Limitation: normal-theory standard errors and chi-square.
        NormalTheoryStandardErrors => "normal_theory_standard_errors",
        /// Limitation: no standardized RMR.
        NoSrmr => "no_srmr",
        /// Limitation: listwise deletion only.
        ListwiseOnly => "listwise_only",
        /// Limitation: each row counts once; no weights.
        NoWeights => "no_weights",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// The confirmatory factor analysis's parameters (decision 745): the factors,
/// each a list of two or more columns in the dataset's order, the factors in
/// the order of their first columns; whether the factors covary; and the
/// missing codes ascending, which apply to every column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CfaParams {
    pub indicators: Vec<Vec<Alias>>,
    pub correlated_factors: bool,
    pub missing_codes: Vec<f64>,
}

impl CfaParams {
    /// Every column, factor by factor, in the spec's order.
    pub fn items(&self) -> Vec<Alias> {
        self.indicators.iter().flatten().copied().collect()
    }
}

/* ------------------------------------------------------------------------ *
 *  The result
 * ------------------------------------------------------------------------ */

/// A number, or the closed reason it is not defined.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Value(f64),
    NotComputable(CfaCode),
}

/// One column's loading on its factor: the estimate, its standard error, C.R.
/// and P, and the standardized weight.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfaLoading {
    pub factor: u32,
    pub item: Alias,
    pub estimate: Stat,
    pub se: Stat,
    pub cr: Stat,
    pub p: Stat,
    pub standardized: Stat,
}

/// One factor's variance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfaFactorVariance {
    pub factor: u32,
    pub estimate: Stat,
    pub se: Stat,
    pub cr: Stat,
    pub p: Stat,
}

/// Two factors' covariance and their correlation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfaFactorCovariance {
    pub factors: [u32; 2],
    pub estimate: Stat,
    pub se: Stat,
    pub cr: Stat,
    pub p: Stat,
    pub correlation: Stat,
}

/// One column's error variance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfaErrorVariance {
    pub item: Alias,
    pub estimate: Stat,
    pub se: Stat,
    pub cr: Stat,
    pub p: Stat,
}

/// One column's squared multiple correlation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfaSmc {
    pub item: Alias,
    pub value: Stat,
}

/// AMOS's measures of fit (its Appendix C; decision 745), and the independence
/// model's NPAR, CMIN and DF. The 90% intervals are `…Lo90` and `…Hi90`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfaFit {
    pub npar: u64,
    pub cmin: Stat,
    pub df: u64,
    pub p: Stat,
    pub cmin_df: Stat,
    pub rmr: Stat,
    pub gfi: Stat,
    pub agfi: Stat,
    pub pgfi: Stat,
    pub nfi: Stat,
    pub rfi: Stat,
    pub ifi: Stat,
    pub tli: Stat,
    pub cfi: Stat,
    pub pratio: Stat,
    pub pnfi: Stat,
    pub pcfi: Stat,
    pub ncp: Stat,
    pub ncp_lo90: Stat,
    pub ncp_hi90: Stat,
    pub fmin: Stat,
    pub f0: Stat,
    pub f0_lo90: Stat,
    pub f0_hi90: Stat,
    pub rmsea: Stat,
    pub rmsea_lo90: Stat,
    pub rmsea_hi90: Stat,
    pub pclose: Stat,
    pub aic: Stat,
    pub bcc: Stat,
    pub bic: Stat,
    pub caic: Stat,
    pub ecvi: Stat,
    pub ecvi_lo90: Stat,
    pub ecvi_hi90: Stat,
    pub mecvi: Stat,
    pub hoelter05: Stat,
    pub hoelter01: Stat,
    pub independence_npar: u64,
    pub independence_cmin: Stat,
    pub independence_df: u64,
}

/// The confirmatory factor analysis's result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CfaOutcome {
    /// Every column, factor by factor, as the spec names them.
    pub items: Vec<Alias>,
    pub rows_used: u64,
    /// The factors' columns, as the spec.
    pub indicators: Vec<Vec<Alias>>,
    /// Every column's loading, the fixed first ones included, in `items`'s
    /// order.
    pub loadings: Vec<CfaLoading>,
    pub factor_variances: Vec<CfaFactorVariance>,
    /// Empty when the factors do not covary.
    pub factor_covariances: Vec<CfaFactorCovariance>,
    pub error_variances: Vec<CfaErrorVariance>,
    pub smc: Vec<CfaSmc>,
    pub fit: CfaFit,
    /// The Fisher scoring steps taken.
    pub iterations: u64,
    /// The rows left out, by column and reason; none with no row is listed.
    pub exclusions: Vec<Exclusion<CfaCode>>,
    pub notes: Vec<CfaCode>,
    pub limitations: Vec<CfaCode>,
}

impl CfaOutcome {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, closed words and aliases serialises")
    }
}

/// What every result says it does not give, in decision 745's order.
const LIMITATIONS: [CfaCode; 10] = [
    CfaCode::MlOnly,
    CfaCode::NoMeans,
    CfaCode::OneGroupOnly,
    CfaCode::SimpleStructureOnly,
    CfaCode::MarkerIdentificationOnly,
    CfaCode::NoModificationIndices,
    CfaCode::NormalTheoryStandardErrors,
    CfaCode::NoSrmr,
    CfaCode::ListwiseOnly,
    CfaCode::NoWeights,
];

/* ------------------------------------------------------------------------ *
 *  The one reading of the rows
 * ------------------------------------------------------------------------ */

/// The complete rows' values, one vector per column in the spec's order, and
/// each column's rows left out. Never leaves this module, and derives nothing.
struct Rows {
    values: Vec<Vec<f64>>,
    /// For each column: rows left out as `missing_value`, as `missing_code`.
    excluded: Vec<[u64; 2]>,
}

impl Rows {
    /// Listwise: a row is used only when every column holds a number that is
    /// not a code; a row left out is counted once, under the first column, in
    /// the dataset's order, without one (decision 745).
    fn of(columns: &[&[Cell]], items: &[Alias], codes: &[f64]) -> Rows {
        let rows = columns.first().map_or(0, |c| c.len());
        let mut out = Rows {
            values: vec![Vec::with_capacity(rows); columns.len()],
            excluded: vec![[0; 2]; columns.len()],
        };
        let mut order: Vec<usize> = (0..columns.len()).collect();
        order.sort_by_key(|j| items[*j]);
        let mut row = vec![0.0; columns.len()];
        'rows: for r in 0..rows {
            for &j in &order {
                match &columns[j][r] {
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

/// Every value equal as doubles, −0 equal to 0.
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
pub fn spec(operation_id: OperationId, dataset: &Dataset, parameters: CfaParams) -> OperationSpec<CfaParams> {
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

fn column_param(alias: Alias) -> ParamValue<CfaCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

struct Checked<'d> {
    errors: Vec<Diagnostic<CfaCode>>,
    ready: Option<Vec<&'d [Cell]>>,
}

/// Shared by `validate` and `run` (decision 745's order): the missing codes;
/// the factors' count; each factor's columns; the columns' count; their order
/// within a factor; the factors' order; a repeated column; each column
/// (unknown, a non-finite value, text); a code equal to a score's value. Then,
/// only when there is no other error, the first of: no complete row, too few
/// rows, a column constant.
fn check<'d>(dataset: &'d Dataset, params: &CfaParams) -> Checked<'d> {
    let mut errors = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(Diagnostic::error(CfaCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64));
        }
    }
    for (i, w) in params.missing_codes.windows(2).enumerate() {
        if w[0] == w[1] {
            errors.push(Diagnostic::error(CfaCode::MissingCodeRepeated).int(ParamKey::Index, i as i64 + 1));
        } else if w[0] > w[1] {
            errors.push(Diagnostic::error(CfaCode::MissingCodesOutOfOrder).int(ParamKey::Index, i as i64 + 1));
        }
    }
    let k = params.indicators.len();
    if k == 0 {
        errors.push(Diagnostic::error(CfaCode::NoFactors));
    } else if k > MAX_FACTORS {
        errors.push(
            Diagnostic::error(CfaCode::TooManyFactors)
                .int(ParamKey::Count, k as i64)
                .int(ParamKey::Limit, MAX_FACTORS as i64),
        );
    }
    for (f, columns) in params.indicators.iter().enumerate() {
        if columns.len() < MIN_FACTOR_ITEMS {
            errors.push(
                Diagnostic::error(CfaCode::FactorTooFewItems)
                    .int(ParamKey::Index, f as i64 + 1)
                    .int(ParamKey::Count, columns.len() as i64)
                    .int(ParamKey::Limit, MIN_FACTOR_ITEMS as i64),
            );
        }
    }
    let items = params.items();
    let p = items.len();
    if p < MIN_ITEMS {
        errors.push(
            Diagnostic::error(CfaCode::TooFewItems)
                .int(ParamKey::Count, p as i64)
                .int(ParamKey::Limit, MIN_ITEMS as i64),
        );
    } else if p > MAX_ITEMS {
        errors.push(
            Diagnostic::error(CfaCode::TooManyItems)
                .int(ParamKey::Count, p as i64)
                .int(ParamKey::Limit, MAX_ITEMS as i64),
        );
    }
    for columns in &params.indicators {
        for w in columns.windows(2) {
            if w[0] > w[1] {
                errors.push(Diagnostic::error(CfaCode::ItemsOutOfOrder).with(ParamKey::Column, column_param(w[1])));
            }
        }
    }
    for (f, w) in params.indicators.windows(2).enumerate() {
        if let (Some(a), Some(b)) = (w[0].first(), w[1].first()) {
            if a > b {
                errors.push(Diagnostic::error(CfaCode::FactorsOutOfOrder).int(ParamKey::Index, f as i64 + 2));
            }
        }
    }
    let mut seen = BTreeSet::new();
    for alias in &items {
        if !seen.insert(*alias) {
            errors.push(Diagnostic::error(CfaCode::ItemRepeated).with(ParamKey::Column, column_param(*alias)));
        }
    }
    let mut cells = Vec::with_capacity(p);
    for alias in &items {
        let Some((summary, column_cells)) = column(dataset, *alias) else {
            errors.push(Diagnostic::error(CfaCode::UnknownColumn).with(ParamKey::Column, column_param(*alias)));
            continue;
        };
        let non_finite = non_finite_cells(summary);
        let texts = column_cells.iter().filter(|c| matches!(c, Cell::Text(_))).count() as i64;
        if non_finite > 0 {
            errors.push(
                Diagnostic::error(CfaCode::ColumnNonFinite)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, non_finite as i64),
            );
        } else if texts > 0 {
            errors.push(
                Diagnostic::error(CfaCode::ColumnHoldsText)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, texts),
            );
        }
        cells.push(column_cells);
    }
    let mut scanned = BTreeSet::new();
    for alias in items.iter().filter(|a| scanned.insert(**a)) {
        let collisions = score_code_count(dataset, *alias, &params.missing_codes);
        if collisions > 0 {
            errors.push(
                Diagnostic::error(CfaCode::CodeEqualsScore)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, collisions as i64),
            );
        }
    }
    if !errors.is_empty() {
        return Checked { errors, ready: None };
    }
    let rows = Rows::of(&cells, &items, &params.missing_codes);
    let n = rows.complete();
    if n == 0 {
        errors.push(Diagnostic::error(CfaCode::NoCompleteRows));
    } else if n < p + 1 {
        errors.push(
            Diagnostic::error(CfaCode::TooFewRows)
                .int(ParamKey::Count, n as i64)
                .int(ParamKey::Limit, (p + 1) as i64),
        );
    } else if let Some(j) = rows.values.iter().position(|x| constant(x)) {
        errors.push(Diagnostic::error(CfaCode::ItemConstant).with(ParamKey::Column, column_param(items[j])));
    }
    Checked { errors, ready: Some(cells) }
}

/// Checks the spec against the dataset. Unsupported when it names another
/// capability, version or workspace; invalid on a wrong input or a parameter
/// error; a warning for each candidate missing code in a column that the user
/// did not list, a score's skipped. Nothing needs input.
pub fn validate(dataset: &Dataset, spec: &OperationSpec<CfaParams>) -> ValidationReport<CfaCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID {
        unsupported.push(Diagnostic::error(CfaCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(CfaCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(CfaCode::InputMismatch));
            }
        }
        inputs => errors.push(Diagnostic::error(CfaCode::InputCount).int(ParamKey::Count, inputs.len() as i64)),
    }
    if !preparation_holds(dataset, spec.preparation.as_ref()) {
        errors.push(Diagnostic::error(CfaCode::PreparationMismatch));
    }
    let params = &spec.parameters;
    errors.extend(check(dataset, params).errors);
    let mut warnings = Vec::new();
    let mut warned = BTreeSet::new();
    for alias in params.items() {
        if !warned.insert(alias) {
            continue;
        }
        if let Some((_, cells)) = column(dataset, alias) {
            for s in suspect_codes_unless_score(dataset, alias, cells) {
                if !params.missing_codes.contains(&f64::from(s.code)) {
                    warnings.push(
                        Diagnostic::warning(CfaCode::SuspectMissingCode)
                            .with(ParamKey::Column, column_param(alias))
                            .int(ParamKey::Value, i64::from(s.code))
                            .int(ParamKey::Count, s.cells as i64),
                    );
                }
            }
        }
    }
    let rows = dataset.rows() as u64;
    let columns = params.items().len() as u64;
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
    Refused(Diagnostic<CfaCode>),
    /// The caller's check point stopped the run.
    Stopped { reason: B },
}

/// How many check points a run of `params` calls (decision 745): one before
/// the arithmetic, one before each column of S, one before the estimation and
/// one before the fit. Every iteration calls the estimation's again, at an
/// unchanged count. Only the spec is needed.
pub fn check_points(params: &CfaParams) -> usize {
    1 + params.items().len() + 2
}

/// The run as a function of the data alone, every check point continuing.
pub fn compute(dataset: &Dataset, params: &CfaParams) -> Result<CfaOutcome, Diagnostic<CfaCode>> {
    compute_with_cap(dataset, params, MAX_ITERATIONS)
}

/// [`compute`] with the iterations' cap as an argument, so a test can reach the
/// cap's refusal on a model that converges (decision 746).
pub fn compute_with_cap(dataset: &Dataset, params: &CfaParams, cap: u32) -> Result<CfaOutcome, Diagnostic<CfaCode>> {
    match run_with_cap(dataset, params, cap, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The confirmatory factor analysis. `checkpoint(0)` is called after the
/// parameters are checked and before the arithmetic, `checkpoint(1..=p)`
/// before each column of S, `checkpoint(p + 1)` before the estimation and
/// again at every iteration, and `checkpoint(p + 2)` before the fit. A `Break`
/// stops the run there. What a stop means is the caller's.
pub fn run<B>(
    dataset: &Dataset,
    params: &CfaParams,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<CfaOutcome, Halt<B>> {
    run_with_cap(dataset, params, MAX_ITERATIONS, checkpoint)
}

/// The model's shape: each column's factor, which columns are markers, and the
/// free parameters' layout: the free loadings (columns in order, markers
/// skipped), the factor variances, the covariances (f < g, in order), the error
/// variances.
struct Model {
    p: usize,
    k: usize,
    factor_of: Vec<usize>,
    marker: Vec<usize>,
    free: Vec<usize>,
    pairs: Vec<(usize, usize)>,
}

impl Model {
    fn of(params: &CfaParams) -> Model {
        let mut factor_of = Vec::new();
        let mut marker = Vec::new();
        let mut free = Vec::new();
        for (f, columns) in params.indicators.iter().enumerate() {
            for (t, _) in columns.iter().enumerate() {
                let i = factor_of.len();
                factor_of.push(f);
                if t == 0 {
                    marker.push(i);
                } else {
                    free.push(i);
                }
            }
        }
        let k = params.indicators.len();
        let pairs = if params.correlated_factors {
            (0..k).flat_map(|f| (f + 1..k).map(move |g| (f, g))).collect()
        } else {
            Vec::new()
        };
        Model { p: factor_of.len(), k, factor_of, marker, free, pairs }
    }

    fn q(&self) -> usize {
        self.free.len() + self.k + self.pairs.len() + self.p
    }

    /// θ unpacked: each column's loading (1 for a marker), Φ, Θ's diagonal.
    fn unpack(&self, theta: &[f64]) -> (Vec<f64>, Vec<Vec<f64>>, Vec<f64>) {
        let mut lambda = vec![1.0; self.p];
        for (t, i) in self.free.iter().enumerate() {
            lambda[*i] = theta[t];
        }
        let base = self.free.len();
        let mut phi = vec![vec![0.0; self.k]; self.k];
        for f in 0..self.k {
            phi[f][f] = theta[base + f];
        }
        for (t, (f, g)) in self.pairs.iter().enumerate() {
            let v = theta[base + self.k + t];
            phi[*f][*g] = v;
            phi[*g][*f] = v;
        }
        let errors = theta[base + self.k + self.pairs.len()..].to_vec();
        (lambda, phi, errors)
    }

    /// Σ(θ) = Λ Φ Λᵀ + Θ.
    fn sigma(&self, theta: &[f64]) -> Vec<Vec<f64>> {
        let (lambda, phi, errors) = self.unpack(theta);
        let mut s = vec![vec![0.0; self.p]; self.p];
        for i in 0..self.p {
            for j in 0..=i {
                let mut v = lambda[i] * lambda[j] * phi[self.factor_of[i]][self.factor_of[j]];
                if i == j {
                    v += errors[i];
                }
                s[i][j] = v;
                s[j][i] = v;
            }
        }
        s
    }
}

/// A vector a Σ_a is built from: a unit vector e_i, Λ's column f, or c_f = Λ
/// Φ's column f.
#[derive(Clone, Copy)]
enum Piece {
    Unit(usize),
    Lambda(usize),
    C(usize),
}

/// What one evaluation at θ gives: Σ's inverse, F, g and H.
struct Evaluation {
    inverse: Vec<Vec<f64>>,
    f: f64,
    g: Vec<f64>,
    h: Vec<Vec<f64>>,
}

/// F at θ, or `None` when Σ(θ) is not positive definite.
fn discrepancy(model: &Model, s: &[Vec<f64>], log_det_s: f64, theta: &[f64]) -> Option<f64> {
    let sigma = model.sigma(theta);
    let l = cholesky_within(&sigma, 0.0).ok()?;
    let inverse = spd_inverse(&l);
    Some(log_det(&l) + trace_product(s, &inverse) - log_det_s - model.p as f64)
}

/// tr(A B) for symmetric A and B, in row order.
fn trace_product(a: &[Vec<f64>], b: &[Vec<f64>]) -> f64 {
    let mut t = 0.0;
    for (ra, rb) in a.iter().zip(b) {
        for (x, y) in ra.iter().zip(rb) {
            t += x * y;
        }
    }
    t
}

/// F, g and H at θ (decision 745), or `None` when Σ(θ) is not positive
/// definite.
fn evaluate(model: &Model, s: &[Vec<f64>], log_det_s: f64, theta: &[f64]) -> Option<Evaluation> {
    let (p, k) = (model.p, model.k);
    let sigma = model.sigma(theta);
    let l = cholesky_within(&sigma, 0.0).ok()?;
    let a = spd_inverse(&l);
    let f = log_det(&l) + trace_product(s, &a) - log_det_s - p as f64;
    let (lambda, phi, _) = model.unpack(theta);
    // R = A − A S A, so g_a = tr(R Σ_a).
    let asa = multiply(&multiply(&a, s), &a);
    let r: Vec<Vec<f64>> = (0..p).map(|i| (0..p).map(|j| a[i][j] - asa[i][j]).collect()).collect();
    // M Λ for M = R and M = A: column f sums M's columns of factor f times λ.
    let times_lambda = |m: &[Vec<f64>]| -> Vec<Vec<f64>> {
        (0..p)
            .map(|i| {
                let mut row = vec![0.0; k];
                for j in 0..p {
                    row[model.factor_of[j]] += m[i][j] * lambda[j];
                }
                row
            })
            .collect()
    };
    let lambda_t = |m: &[Vec<f64>]| -> Vec<Vec<f64>> {
        let mut out = vec![vec![0.0; m[0].len()]; k];
        for i in 0..p {
            for (g, x) in m[i].iter().enumerate() {
                out[model.factor_of[i]][g] += lambda[i] * x;
            }
        }
        out
    };
    let r_lambda = times_lambda(&r);
    let lrl = lambda_t(&r_lambda);
    let a_lambda = times_lambda(&a);
    let lal = lambda_t(&a_lambda);
    let a_c = multiply(&a_lambda, &phi);
    let lal_phi = multiply(&lal, &phi);
    let phi_lal_phi = multiply(&phi, &lal_phi);
    // The gradient.
    let q = model.q();
    let mut g = Vec::with_capacity(q);
    for i in &model.free {
        let f = model.factor_of[*i];
        let rc: f64 = (0..k).map(|h| r_lambda[*i][h] * phi[h][f]).sum();
        g.push(2.0 * rc);
    }
    for f in 0..k {
        g.push(lrl[f][f]);
    }
    for (f, h) in &model.pairs {
        g.push(2.0 * lrl[*f][*h]);
    }
    for i in 0..p {
        g.push(r[i][i]);
    }
    // The information: each parameter's Σ_a as pieces u vᵀ.
    let mut pieces: Vec<Vec<(Piece, Piece)>> = Vec::with_capacity(q);
    for i in &model.free {
        let f = model.factor_of[*i];
        pieces.push(vec![(Piece::Unit(*i), Piece::C(f)), (Piece::C(f), Piece::Unit(*i))]);
    }
    for f in 0..k {
        pieces.push(vec![(Piece::Lambda(f), Piece::Lambda(f))]);
    }
    for (f, h) in &model.pairs {
        pieces.push(vec![(Piece::Lambda(*f), Piece::Lambda(*h)), (Piece::Lambda(*h), Piece::Lambda(*f))]);
    }
    for i in 0..p {
        pieces.push(vec![(Piece::Unit(i), Piece::Unit(i))]);
    }
    // xᵀ A y for two pieces.
    let form = |x: Piece, y: Piece| -> f64 {
        match (x, y) {
            (Piece::Unit(i), Piece::Unit(j)) => a[i][j],
            (Piece::Unit(i), Piece::Lambda(f)) | (Piece::Lambda(f), Piece::Unit(i)) => a_lambda[i][f],
            (Piece::Unit(i), Piece::C(f)) | (Piece::C(f), Piece::Unit(i)) => a_c[i][f],
            (Piece::Lambda(f), Piece::Lambda(h)) => lal[f][h],
            (Piece::Lambda(f), Piece::C(h)) => lal_phi[f][h],
            (Piece::C(f), Piece::Lambda(h)) => lal_phi[h][f],
            (Piece::C(f), Piece::C(h)) => phi_lal_phi[f][h],
        }
    };
    let mut h = vec![vec![0.0; q]; q];
    for x in 0..q {
        for y in 0..=x {
            let mut v = 0.0;
            for (us, vs) in &pieces[x] {
                for (ut, vt) in &pieces[y] {
                    v += form(*vs, *ut) * form(*vt, *us);
                }
            }
            h[x][y] = v;
            h[y][x] = v;
        }
    }
    Some(Evaluation { inverse: a, f, g, h })
}

/// H⁻¹ g, as a vector.
fn solve(h_inverse: &[Vec<f64>], g: &[f64]) -> Vec<f64> {
    h_inverse.iter().map(|row| row.iter().zip(g).map(|(x, y)| x * y).sum()).collect()
}

/// The starts of decision 745: φ_ff = s_mm / 2, λ_i = s_im / φ_ff, θ_ii = s_ii /
/// 2, φ_fg = s_{m_f m_g} / 2; with `ones`, every free loading 1.
fn starts(model: &Model, s: &[Vec<f64>], ones: bool) -> Vec<f64> {
    let mut theta = Vec::with_capacity(model.q());
    for i in &model.free {
        let m = model.marker[model.factor_of[*i]];
        theta.push(if ones { 1.0 } else { s[*i][m] / (s[m][m] / 2.0) });
    }
    for f in 0..model.k {
        let m = model.marker[f];
        theta.push(s[m][m] / 2.0);
    }
    for (f, g) in &model.pairs {
        theta.push(s[model.marker[*f]][model.marker[*g]] / 2.0);
    }
    for i in 0..model.p {
        theta.push(s[i][i] / 2.0);
    }
    theta
}

/// The solution, Σ̂'s inverse and H⁻¹ there, and the steps taken.
struct Fitted {
    theta: Vec<f64>,
    inverse: Vec<Vec<f64>>,
    f: f64,
    h_inverse: Vec<Vec<f64>>,
    iterations: u64,
}

/// Fisher scoring from `theta` (decision 745). `Err((code, iteration))`: the
/// run's refusal and the iteration it came at, 0 being the start.
fn estimate<B>(
    model: &Model,
    s: &[Vec<f64>],
    log_det_s: f64,
    mut theta: Vec<f64>,
    cap: u32,
    at: &mut impl FnMut() -> Result<(), Halt<B>>,
) -> Result<Result<Fitted, (CfaCode, u64)>, Halt<B>> {
    let mut iterations = 0u64;
    loop {
        at()?;
        let Some(e) = evaluate(model, s, log_det_s, &theta) else {
            return Ok(Err((CfaCode::CfaNotConverged, iterations)));
        };
        let Ok(lh) = cholesky(&e.h) else {
            return Ok(Err((CfaCode::ModelNotIdentified, iterations)));
        };
        let h_inverse = spd_inverse(&lh);
        let step: Vec<f64> = solve(&h_inverse, &e.g).iter().map(|x| -x).collect();
        let decrement: f64 = -e.g.iter().zip(&step).map(|(x, y)| x * y).sum::<f64>();
        if decrement <= DECREMENT_STOP {
            return Ok(Ok(Fitted { theta, inverse: e.inverse, f: e.f, h_inverse, iterations }));
        }
        if iterations >= u64::from(cap) {
            return Ok(Err((CfaCode::CfaNotConverged, iterations)));
        }
        let mut alpha = 1.0;
        let mut next = None;
        for _ in 0..=MAX_HALVINGS {
            let candidate: Vec<f64> = theta.iter().zip(&step).map(|(t, d)| t + alpha * d).collect();
            if let Some(f) = discrepancy(model, s, log_det_s, &candidate) {
                if decrement <= DECREMENT_F_TEST || f <= e.f {
                    next = Some(candidate);
                    break;
                }
            }
            alpha /= 2.0;
        }
        let Some(candidate) = next else {
            return Ok(Err((CfaCode::CfaNotConverged, iterations)));
        };
        theta = candidate;
        iterations += 1;
    }
}

/// The model's degrees of freedom, p (p + 1) / 2 − q; `Err` with the count by
/// which q exceeds the moments, which [`run_with_cap`] refuses as
/// `model_not_identified` before estimating (decision 745). Its own function so
/// the unit test reaches the guard the run uses (the review of 9C-1, finding 11).
fn degrees_of_freedom(model: &Model) -> Result<usize, usize> {
    let moments = model.p * (model.p + 1) / 2;
    let q = model.q();
    if q > moments {
        Err(q - moments)
    } else {
        Ok(moments - q)
    }
}

/// [`run`] with the iterations' cap as an argument.
fn run_with_cap<B>(
    dataset: &Dataset,
    params: &CfaParams,
    cap: u32,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<CfaOutcome, Halt<B>> {
    let checked = check(dataset, params);
    if let Some(e) = checked.errors.into_iter().next() {
        return Err(Halt::Refused(e));
    }
    let cells = checked.ready.expect("no error means every column is good");
    let mut at = |point: usize| match checkpoint(point) {
        ControlFlow::Break(reason) => Err(Halt::Stopped { reason }),
        ControlFlow::Continue(()) => Ok(()),
    };
    at(0)?;
    let items = params.items();
    let rows = Rows::of(&cells, &items, &params.missing_codes);
    let p = items.len();
    let n = rows.complete();
    let nf = n as f64;
    // S with divisor N, from two-pass means, column by column.
    let means: Vec<f64> = rows.values.iter().map(|x| x.iter().sum::<f64>() / nf).collect();
    let mut s = vec![vec![0.0; p]; p];
    for i in 0..p {
        at(1 + i)?;
        for j in 0..=i {
            let mut v = 0.0;
            for r in 0..n {
                v += (rows.values[i][r] - means[i]) * (rows.values[j][r] - means[j]);
            }
            s[i][j] = v / nf;
            s[j][i] = s[i][j];
        }
    }
    let ls = cholesky(&s).map_err(|j| {
        Halt::Refused(Diagnostic::error(CfaCode::SampleCovarianceSingular).with(ParamKey::Column, column_param(items[j - 1])))
    })?;
    let log_det_s = log_det(&ls);
    let model = Model::of(params);
    if let Err(count) = degrees_of_freedom(&model) {
        return Err(Halt::Refused(Diagnostic::error(CfaCode::ModelNotIdentified).int(ParamKey::Count, count as i64)));
    }
    let refuse = |code: CfaCode| -> Halt<B> {
        match code {
            CfaCode::CfaNotConverged => Halt::Refused(Diagnostic::error(code).int(ParamKey::Limit, i64::from(cap))),
            _ => Halt::Refused(Diagnostic::error(code)),
        }
    };
    let estimation_point = 1 + p;
    let mut at_estimation = || at(estimation_point);
    let first = starts(&model, &s, false);
    let any_zero = first[..model.free.len()].iter().any(|x| *x == 0.0);
    let fitted = match estimate(&model, &s, log_det_s, first, cap, &mut at_estimation)? {
        Ok(f) => f,
        // Decision 745: a start loading of exactly 0 and H failing at the start:
        // once more from every free loading 1.
        Err((CfaCode::ModelNotIdentified, 0)) if any_zero => {
            match estimate(&model, &s, log_det_s, starts(&model, &s, true), cap, &mut at_estimation)? {
                Ok(f) => f,
                Err((code, _)) => return Err(refuse(code)),
            }
        }
        Err((code, _)) => return Err(refuse(code)),
    };
    at(p + 2)?;
    Ok(outcome(params, &items, &rows, &model, &s, log_det_s, &fitted))
}

/* ------------------------------------------------------------------------ *
 *  The statistics and the fit
 * ------------------------------------------------------------------------ */

/// `v` as a value; an infinity is `overflow`; a NaN, a 0/0 of quantities that
/// vanished, is `too_small`.
fn stat(v: f64) -> Stat {
    if v.is_finite() {
        Stat::Value(v)
    } else if v.is_nan() {
        Stat::NotComputable(CfaCode::TooSmall)
    } else {
        Stat::NotComputable(CfaCode::Overflow)
    }
}

/// A distribution's value, or `distribution_out_of_range` where it gives none.
fn dist(v: Option<f64>) -> Stat {
    v.map_or(Stat::NotComputable(CfaCode::DistributionOutOfRange), stat)
}

/// `v` unless the degrees of freedom are 0.
fn unless_df_zero(d: u64, v: impl FnOnce() -> Stat) -> Stat {
    if d == 0 {
        Stat::NotComputable(CfaCode::DfZero)
    } else {
        v()
    }
}

/// An estimate with its standard error, C.R. and two-tailed P.
fn tested(estimate: f64, se: f64) -> (Stat, Stat, Stat, Stat) {
    let cr = estimate / se;
    (stat(estimate), stat(se), stat(cr), stat(normal_two_tailed(cr)))
}

/// Every statistic of decision 745 at the solution.
fn outcome(
    params: &CfaParams,
    items: &[Alias],
    rows: &Rows,
    model: &Model,
    s: &[Vec<f64>],
    log_det_s: f64,
    fitted: &Fitted,
) -> CfaOutcome {
    let (p, k) = (model.p, model.k);
    let n_rows = rows.complete();
    let big_n = n_rows as f64;
    let n = big_n - 1.0;
    let theta = &fitted.theta;
    let (lambda, phi, errors) = model.unpack(theta);
    let sigma = model.sigma(theta);
    // The standard errors: √ diag [(N − 1) H / 2]⁻¹ = √ (2 (H⁻¹)_aa / (N − 1)).
    let se: Vec<f64> = (0..model.q()).map(|a| (2.0 * fitted.h_inverse[a][a] / n).sqrt()).collect();
    let base = model.free.len();
    let negative = Stat::NotComputable(CfaCode::NegativeVariance);
    let fixed = Stat::NotComputable(CfaCode::FixedParameter);

    let mut loadings = Vec::with_capacity(p);
    for i in 0..p {
        let f = model.factor_of[i];
        let standardized = if phi[f][f] > 0.0 && sigma[i][i] > 0.0 {
            stat(lambda[i] * phi[f][f].sqrt() / sigma[i][i].sqrt())
        } else {
            negative
        };
        let (estimate, se_i, cr, pv) = match model.free.iter().position(|x| *x == i) {
            Some(t) => tested(lambda[i], se[t]),
            None => (stat(1.0), fixed, fixed, fixed),
        };
        loadings.push(CfaLoading { factor: f as u32 + 1, item: items[i], estimate, se: se_i, cr, p: pv, standardized });
    }
    let factor_variances = (0..k)
        .map(|f| {
            let (estimate, se_f, cr, pv) = tested(phi[f][f], se[base + f]);
            CfaFactorVariance { factor: f as u32 + 1, estimate, se: se_f, cr, p: pv }
        })
        .collect();
    let factor_covariances = model
        .pairs
        .iter()
        .enumerate()
        .map(|(t, (f, g))| {
            let (estimate, se_c, cr, pv) = tested(phi[*f][*g], se[base + k + t]);
            let correlation = if phi[*f][*f] > 0.0 && phi[*g][*g] > 0.0 {
                stat(phi[*f][*g] / (phi[*f][*f] * phi[*g][*g]).sqrt())
            } else {
                negative
            };
            CfaFactorCovariance { factors: [*f as u32 + 1, *g as u32 + 1], estimate, se: se_c, cr, p: pv, correlation }
        })
        .collect();
    let error_base = base + k + model.pairs.len();
    let error_variances = (0..p)
        .map(|i| {
            let (estimate, se_e, cr, pv) = tested(errors[i], se[error_base + i]);
            CfaErrorVariance { item: items[i], estimate, se: se_e, cr, p: pv }
        })
        .collect();
    let smc = (0..p)
        .map(|i| CfaSmc {
            item: items[i],
            value: if sigma[i][i] > 0.0 { stat(1.0 - errors[i] / sigma[i][i]) } else { negative },
        })
        .collect();

    let fit = fit_measures(model, s, log_det_s, &sigma, fitted, big_n, n);

    let mut notes = Vec::new();
    if (0..k).any(|f| phi[f][f] < 0.0) || errors.iter().any(|e| *e < 0.0) {
        notes.push(CfaCode::NegativeVariance);
    }
    if cholesky_within(&phi, 0.0).is_err() {
        notes.push(CfaCode::FactorCovarianceNotPositiveDefinite);
    }
    if fit.df == 0 {
        notes.push(CfaCode::DfZero);
    }

    let mut exclusions = Vec::new();
    let mut order: Vec<usize> = (0..p).collect();
    order.sort_by_key(|j| items[*j]);
    for j in order {
        for (code, count) in [(CfaCode::MissingValue, rows.excluded[j][0]), (CfaCode::MissingCode, rows.excluded[j][1])] {
            if count > 0 {
                exclusions.push(Exclusion {
                    code,
                    count,
                    params: BTreeMap::from([(ParamKey::Column, column_param(items[j]))]),
                });
            }
        }
    }

    CfaOutcome {
        items: items.to_vec(),
        rows_used: n_rows as u64,
        indicators: params.indicators.clone(),
        loadings,
        factor_variances,
        factor_covariances,
        error_variances,
        smc,
        fit,
        iterations: fitted.iterations,
        exclusions,
        notes,
        limitations: LIMITATIONS.to_vec(),
    }
}

/// AMOS's Appendix C (decision 745), with C = CMIN, d = DF, q = NPAR, n = N − 1,
/// p* = p (p + 1) / 2 and b the independence model.
fn fit_measures(model: &Model, s: &[Vec<f64>], log_det_s: f64, sigma: &[Vec<f64>], fitted: &Fitted, big_n: f64, n: f64) -> CfaFit {
    let p = model.p;
    let pf = p as f64;
    let q = model.q() as u64;
    let qf = q as f64;
    let moments = (p * (p + 1) / 2) as u64;
    let d = moments - q;
    let df = d as f64;
    let pstar = moments as f64;
    let c = n * fitted.f.max(0.0);
    // The independence model: only the variances free.
    let qb = p as u64;
    let db = (p * (p - 1) / 2) as u64;
    let dbf = db as f64;
    let ln_diag: f64 = (0..p).map(|i| s[i][i].ln()).sum();
    let cb = n * (ln_diag - log_det_s);
    // RMR over the lower triangle with the diagonal, on S with divisor N.
    let mut squares = 0.0;
    for i in 0..p {
        for j in 0..=i {
            let r = s[i][j] - sigma[i][j];
            squares += r * r;
        }
    }
    let rmr = (squares / pstar).sqrt();
    // GFI from Σ̂⁻¹ S.
    let m = multiply(&fitted.inverse, s);
    let (mut residual, mut whole) = (0.0, 0.0);
    for i in 0..p {
        for j in 0..p {
            let a = m[i][j] - if i == j { 1.0 } else { 0.0 };
            let b = m[j][i] - if i == j { 1.0 } else { 0.0 };
            residual += a * b;
            whole += m[i][j] * m[j][i];
        }
    }
    let gfi = 1.0 - residual / whole;
    let ncp = (c - df).max(0.0);
    let lo = if d == 0 { None } else { noncentral_chi_square_ncp(c, df, 0.95) };
    let hi = if d == 0 { None } else { noncentral_chi_square_ncp(c, df, 0.05) };
    let interval = |v: Option<f64>, f: &dyn Fn(f64) -> f64| unless_df_zero(d, || dist(v.map(f)));
    let nfi = 1.0 - c / cb;
    let cfi_denominator = (cb - dbf).max(c - df).max(0.0);
    let cfi = if cfi_denominator == 0.0 { 1.0 } else { 1.0 - ncp / cfi_denominator };
    let pratio = df / dbf;
    let bcc = if big_n <= pf + 2.0 {
        Stat::NotComputable(CfaCode::TooFewRowsForBcc)
    } else {
        stat(c + 2.0 * qf * (big_n - 1.0) / (big_n - pf - 2.0))
    };
    let hoelter = |alpha: f64| {
        unless_df_zero(d, || {
            if c == 0.0 {
                Stat::NotComputable(CfaCode::CminZero)
            } else {
                match chi_square_quantile_upper(alpha, df) {
                    Some(x) => stat((n * x / c + 1.0).floor()),
                    None => Stat::NotComputable(CfaCode::DistributionOutOfRange),
                }
            }
        })
    };
    CfaFit {
        npar: q,
        cmin: stat(c),
        df: d,
        p: unless_df_zero(d, || dist(chi_square_upper_wide(c, df))),
        cmin_df: unless_df_zero(d, || stat(c / df)),
        rmr: stat(rmr),
        gfi: stat(gfi),
        agfi: unless_df_zero(d, || stat(1.0 - (pstar / df) * (1.0 - gfi))),
        pgfi: unless_df_zero(d, || stat((df / pstar) * gfi)),
        nfi: stat(nfi),
        rfi: unless_df_zero(d, || stat(1.0 - (c / df) / (cb / dbf))),
        ifi: stat((cb - c) / (cb - df)),
        tli: unless_df_zero(d, || stat((cb / dbf - c / df) / (cb / dbf - 1.0))),
        cfi: stat(cfi),
        pratio: unless_df_zero(d, || stat(pratio)),
        pnfi: unless_df_zero(d, || stat(nfi * pratio)),
        pcfi: unless_df_zero(d, || stat(cfi * pratio)),
        ncp: stat(ncp),
        ncp_lo90: interval(lo, &|x| x),
        ncp_hi90: interval(hi, &|x| x),
        fmin: stat(c / n),
        f0: stat(ncp / n),
        f0_lo90: interval(lo, &|x| x / n),
        f0_hi90: interval(hi, &|x| x / n),
        rmsea: unless_df_zero(d, || stat((ncp / n / df).sqrt())),
        rmsea_lo90: interval(lo, &|x| (x / (n * df)).sqrt()),
        rmsea_hi90: interval(hi, &|x| (x / (n * df)).sqrt()),
        pclose: unless_df_zero(d, || {
            dist(noncentral_chi_square_tails(c, df, PCLOSE_RMSEA * PCLOSE_RMSEA * n * df).map(|t| t.1))
        }),
        aic: stat(c + 2.0 * qf),
        bcc,
        bic: stat(c + qf * big_n.ln()),
        caic: stat(c + qf * (big_n.ln() + 1.0)),
        ecvi: stat((c + 2.0 * qf) / n),
        ecvi_lo90: interval(lo, &|x| (x + df + 2.0 * qf) / n),
        ecvi_hi90: interval(hi, &|x| (x + df + 2.0 * qf) / n),
        mecvi: match bcc {
            Stat::Value(v) => stat(v / n),
            code => code,
        },
        hoelter05: hoelter(0.05),
        hoelter01: hoelter(0.01),
        independence_npar: qb,
        independence_cmin: stat(cb),
        independence_df: db,
    }
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows used
/// are the complete rows; with the exclusions they add up to the data rows. The
/// output hash is SHA-256 of [`CfaOutcome::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<CfaParams>,
    dataset: &Dataset,
    result: &CfaOutcome,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<CfaCode> {
    let parameters = serde_json::to_vec(&spec.parameters).expect("parameters of aliases and numbers serialise");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn a(n: u32) -> Alias {
        Alias(crate::stats_l1::dataset::ColumnAlias(n))
    }

    /// Decision 745: DF is never below 0 within the spec's limits, so the guard
    /// on q > p (p + 1) / 2 is reached only by a model the checks refuse first:
    /// one factor of two columns has 4 parameters and 3 moments, so its count
    /// is 1.
    #[test]
    fn a_model_with_more_parameters_than_moments_counts_them() {
        let params = CfaParams { indicators: vec![vec![a(1), a(2)]], correlated_factors: true, missing_codes: vec![] };
        let model = Model::of(&params);
        let moments = model.p * (model.p + 1) / 2;
        assert_eq!((model.q(), moments), (4, 3));
        assert_eq!(degrees_of_freedom(&model), Err(1));
        // Within the limits the smallest models: one factor of three columns at
        // DF 0, two correlated factors of two columns at DF 1.
        let three = Model::of(&CfaParams { indicators: vec![vec![a(1), a(2), a(3)]], correlated_factors: true, missing_codes: vec![] });
        assert_eq!(degrees_of_freedom(&three), Ok(0));
        let two_two = Model::of(&CfaParams { indicators: vec![vec![a(1), a(2)], vec![a(3), a(4)]], correlated_factors: true, missing_codes: vec![] });
        assert_eq!(degrees_of_freedom(&two_two), Ok(1));
    }

    /// The line search's test of a trial point (decision 745): a θ whose Σ̂ is
    /// not positive definite has no discrepancy, so the step is halved. Rounding
    /// at a Cholesky tolerance below 0 would instead give a NaN that a step at a
    /// decrement of at most 1e-10 accepts (the review of 9C-1's surviving
    /// mutant m_line_search_no_pd_test). Only the last pivot fails here, below
    /// 0 while its diagonal stays above 0 (θ66 = −0.4, σ̂66 = 0.09): at a
    /// tolerance below 0 an earlier bad pivot's NaN makes every later pivot fail
    /// anyway, and a diagonal below 0 turns −∞ times it into +∞, so neither
    /// tells the two tolerances apart (measured: two earlier versions of this
    /// test, Φ indefinite and then θ66 = −10, stayed green against that mutant).
    #[test]
    fn a_trial_point_whose_sigma_is_not_positive_definite_has_no_discrepancy() {
        let params = CfaParams {
            indicators: vec![vec![a(1), a(2), a(3)], vec![a(4), a(5), a(6)]],
            correlated_factors: true,
            missing_codes: vec![],
        };
        let model = Model::of(&params);
        let theta = vec![0.8, 0.9, 1.1, 0.7, 1.0, 1.0, 0.1, 4.0, 4.0, 4.0, 4.0, 4.0, -0.4];
        let sigma = model.sigma(&theta);
        assert!(sigma[5][5] > 0.0, "the premise: the last diagonal is above 0");
        assert_eq!(cholesky_within(&sigma, 0.0), Err(6), "the premise: only the last pivot fails");
        let s: Vec<Vec<f64>> = (0..6).map(|i| (0..6).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
        assert_eq!(discrepancy(&model, &s, 0.0, &theta), None);
    }

    /// Decision 745's negative_variance codes for a standardized weight and a
    /// correlation. The estimation reaches a factor variance below 0 only
    /// rarely: on two datasets of 13 Nov 2026 the path met a singular
    /// information matrix first and the run refused as model_not_identified,
    /// while the bug hunt's h043 ends at φ11 = −2,512 (the review of 9C-1; this
    /// comment said no dataset reached it). No fixture holds such data, so the
    /// rule is checked here on a solution built for it: factor
    /// 1's variance below 0, Σ positive definite through large error variances,
    /// S equal to Σ.
    #[test]
    fn a_factor_variance_below_0_leaves_its_standardized_weights_and_correlations_not_computable() {
        let params = CfaParams {
            indicators: vec![vec![a(1), a(2), a(3)], vec![a(4), a(5), a(6)]],
            correlated_factors: true,
            missing_codes: vec![],
        };
        let model = Model::of(&params);
        // λ for columns 2, 3, 5, 6; φ11 = −0.2, φ22 = 1; φ12 = 0.1; Θ = 4.
        let theta = vec![0.8, 0.9, 1.1, 0.7, -0.2, 1.0, 0.1, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0];
        let sigma = model.sigma(&theta);
        let l = cholesky_within(&sigma, 0.0).expect("Σ is positive definite");
        let rows = Rows { values: vec![vec![0.0; 10]; 6], excluded: vec![[0, 0]; 6] };
        let q = model.q();
        let identity: Vec<Vec<f64>> = (0..q).map(|i| (0..q).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
        let fitted = Fitted { theta, inverse: spd_inverse(&l), f: 0.0, h_inverse: identity, iterations: 0 };
        let items = params.items();
        let r = outcome(&params, &items, &rows, &model, &sigma, log_det(&l), &fitted);
        let negative = Stat::NotComputable(CfaCode::NegativeVariance);
        for loading in &r.loadings[..3] {
            assert_eq!(loading.standardized, negative);
        }
        assert!(matches!(r.loadings[3].standardized, Stat::Value(_)));
        assert_eq!(r.factor_covariances[0].correlation, negative);
        assert_eq!(r.notes, vec![CfaCode::NegativeVariance, CfaCode::FactorCovarianceNotPositiveDefinite]);
        // The SMCs are given wherever σ̂_ii > 0, below 0 here for factor 1's columns.
        assert!(matches!(r.smc[0].value, Stat::Value(x) if x < 0.0));
    }
}
