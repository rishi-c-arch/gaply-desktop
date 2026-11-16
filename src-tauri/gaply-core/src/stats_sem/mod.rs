//! **`gaply_core::stats_sem`: structural equation models on the user's data, as
//! AMOS fits a path diagram (Research OS, Phase 9C-4, decisions 795 to 799).**
//! Factors each measured by two or more numeric columns, single-headed arrows
//! (paths) between factors and the columns no factor holds, loops among them,
//! double-headed arrows (covariances) between exogenous variables or error
//! terms, and parameters held equal, as AMOS's shared labels hold them. No
//! means are estimated.
//!
//! **The model (decision 795) is RAM's**, the Reticular Action Model: every
//! variable, observed or not, is a node; A holds every single-headed arrow, a
//! factor's first loading fixed at 1; Φ holds each exogenous variable's
//! variance, each endogenous variable's error variance and every covariance.
//! Σ = F (I − A)⁻¹ Φ (I − A)⁻ᵀ Fᵀ, F selecting the columns. AMOS's conventions
//! are the confirmatory factor analysis's (decision 743): S with divisor N; F(θ)
//! = ln|Σ(θ)| + tr(S Σ(θ)⁻¹) − ln|S| − p; CMIN = (N − 1) F at its minimum;
//! standard errors √(2 (H⁻¹)_aa / (N − 1)) from the expected information H.
//!
//! **Identification (decision 798)** is checked from the spec alone, before any
//! data: H at two points drawn from the spec, scaled to a unit diagonal, its
//! rank by a Cholesky factor that pivots on the largest remaining diagonal; a
//! model whose rank is below its parameters is refused with the count of
//! constraints it lacks, as AMOS's message gives it. At the solution, an
//! estimate the data barely determine refuses the run
//! (`solution_not_identified`).
//!
//! **The estimation (decision 798)**: three attempts, each to a cap: Fisher
//! scoring from the starts; Newton's method on the exact second derivatives,
//! damped, from the same starts, whose stop is a solution only where the
//! observed information is positive definite (a minimum, not a saddle); and
//! Fisher scoring from the triad starts.
//!
//! **The statistics (decision 799)**: AMOS's tables of regression weights,
//! covariances, variances, squared multiple correlations, the total, direct and
//! indirect effects among the structural variables, standardized and not, and
//! the fit's three rows (`stats_fit`, decision 796). Not computable, statistic by
//! statistic, as a closed code; notes are bare codes; no word says a fit is
//! good, poor, close, acceptable or adequate, and no cutoff is named.
//!
//! **What never leaves.** Factors are numbered 1 to k; no name a user typed is
//! in the spec, the result or a line written anywhere (ADR-011). The result
//! holds no cell, no header and no p × p matrix: numbers, closed words and
//! aliases.
//!
//! Pure: no file, network, process, clock, model or log
//! (`tests/contracts_are_pure.rs`); every operation +, −, ×, ÷ and √, but ln in
//! `log_det` and the fit. lavaan 0.7-3, refined by Fisher scoring in R written
//! from these formulas, is the oracle (`tests/fixtures/stats_sem/
//! make_sem_fixtures.R`, compared in `tests/sem_r_golden.rs`); AMOS's printed
//! Examples 6, 7 and 8 are the second (`tests/sem_amos.rs`). Nothing of
//! lavaan's or AMOS's code is in Gaply.

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
use crate::stats_dist::normal_two_tailed;
use crate::stats_fit::{fit_measures as fit_rows, model_df, FitCode, FitInput, FitPoint, FitRow, FitValue};
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_linalg::{cholesky, cholesky_within, inverse, log_det, multiply, pivoted_rank, spd_inverse};
use crate::stats_ttest::{column, non_finite_cells};

pub mod export;

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.sem";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;

/// The fewest columns in a model.
pub const MIN_COLUMNS: usize = 2;
/// The most columns in a model (decision 797).
pub const MAX_COLUMNS: usize = 128;
/// The most factors (decision 797).
pub const MAX_FACTORS: usize = 16;
/// The fewest columns measuring one factor.
pub const MIN_FACTOR_ITEMS: usize = 2;
/// The most paths (decision 797).
pub const MAX_PATHS: usize = 64;
/// The most covariances (decision 797).
pub const MAX_COVARIANCES: usize = 256;
/// The most members over every group of parameters held equal (decision 797).
pub const MAX_EQUAL_MEMBERS: usize = 128;
/// The fewest members of one group held equal.
pub const MIN_EQUAL_MEMBERS: usize = 2;
/// Each attempt's cap on iterations (decision 798).
pub const MAX_ITERATIONS: u32 = 500;
/// An attempt stops when the Newton decrement gᵀH⁻¹g is at most this (decision
/// 798, as decision 745).
pub const DECREMENT_STOP: f64 = 1e-20;
/// Above this decrement a Fisher step must not increase F (decision 745).
pub const DECREMENT_F_TEST: f64 = 1e-10;
/// The halvings of a Fisher step before its attempt ends (decision 745).
pub const MAX_HALVINGS: u32 = 30;
/// The solves of one Newton step before its attempt ends (decision 798).
pub const MAX_SOLVES: u32 = 60;
/// The damping a Newton step starts from when J + μ D is refused at μ = 0, or
/// F there is undefined or larger (decision 798).
pub const DAMPING_START: f64 = 1e-4;
/// J + μ D must pass its Cholesky factor at this tolerance (decision 798).
pub const DAMPED_TOLERANCE: f64 = 1e-12;
/// D's entries are H's diagonal, each raised to at least this times its
/// largest (decision 798).
pub const DIAGONAL_FLOOR: f64 = 1e-12;
/// A pivot of the scaled information above this counts towards its rank
/// (decision 798).
pub const IDENTIFICATION_TOLERANCE: f64 = 1e-9;
/// The seeds of the identification check's two points (decision 798).
pub const IDENTIFICATION_SEEDS: [u64; 2] = [0x9C4, 0x9C5];
/// At the solution, √((H⁻¹)_aa) over a parameter's natural unit above this
/// refuses the run as `solution_not_identified` (decision 798).
pub const SOLUTION_LIMIT: f64 = 1e8;

closed! {
    /// Every refusal, outcome, exclusion, note and limitation of the structural
    /// equation models.
    pub enum SemCode in "stats.sem." {
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
        /// The model names no factor, path or covariance.
        EmptyModel => "empty_model",
        /// More factors than the limit. Params `count`, `limit`.
        TooManyFactors => "too_many_factors",
        /// A factor is measured by fewer than two columns. Params `index` (the
        /// factor's number), `count`, `limit`.
        FactorTooFewItems => "factor_too_few_items",
        /// A factor's columns are not in the dataset's order. Param `column`.
        ItemsOutOfOrder => "items_out_of_order",
        /// The factors are not in the order of their first columns. Param
        /// `index` (the factor's number).
        FactorsOutOfOrder => "factors_out_of_order",
        /// A column measures two factors, or one twice. Param `column`.
        ItemRepeated => "item_repeated",
        /// More paths than the limit. Params `count`, `limit`.
        TooManyPaths => "too_many_paths",
        /// A path or covariance names a factor number outside 1 to k. Param
        /// `index` (its position in its list, from 1).
        UnknownFactor => "unknown_factor",
        /// A path from a variable to itself. Param `index`.
        PathToItself => "path_to_itself",
        /// A path with a factor's column at either end. Param `column`.
        IndicatorInPath => "indicator_in_path",
        /// The paths are not ascending by (to, from). Param `index`.
        PathsOutOfOrder => "paths_out_of_order",
        /// A path is listed twice. Param `index`.
        PathRepeated => "path_repeated",
        /// More covariances than the limit. Params `count`, `limit`.
        TooManyCovariances => "too_many_covariances",
        /// A covariance of a variable with itself. Param `index`.
        CovarianceWithItself => "covariance_with_itself",
        /// A covariance whose ends are not in order, or the covariances not
        /// ascending by (a, b). Param `index`.
        CovariancesOutOfOrder => "covariances_out_of_order",
        /// A covariance is listed twice. Param `index`.
        CovarianceRepeated => "covariance_repeated",
        /// More members over every group held equal than the limit. Params
        /// `count`, `limit`.
        TooManyEqualMembers => "too_many_equal_members",
        /// A group held equal has fewer than two members. Params `index` (the
        /// group's position, from 1), `count`, `limit`.
        EqualGroupTooSmall => "equal_group_too_small",
        /// A group holds parameters of more than one kind. Param `index`.
        EqualMixedKinds => "equal_mixed_kinds",
        /// A member names a loading, path, covariance or variable the model
        /// does not have. Params `index` and `value` (the member's position in
        /// its group, from 1).
        EqualUnknownParameter => "equal_unknown_parameter",
        /// A member is a first loading, fixed at 1. Params `index`, `value`.
        EqualFixedParameter => "equal_fixed_parameter",
        /// A group's members are not ascending (params `index`, `value`), or
        /// the groups are not ascending by their first members (param
        /// `index`).
        EqualOutOfOrder => "equal_out_of_order",
        /// A parameter is held in a group twice, or in two groups. Params
        /// `index`, `value`.
        EqualRepeated => "equal_repeated",
        /// Fewer columns in the model than the limit. Params `count`, `limit`.
        TooFewColumns => "too_few_columns",
        /// More columns in the model than the limit. Params `count`, `limit`.
        TooManyColumns => "too_many_columns",
        /// A column the dataset does not have. Param `column`.
        UnknownColumn => "unknown_column",
        /// A column holds NaN or an infinity. Params `column`, `count`.
        ColumnNonFinite => "column_non_finite",
        /// A column holds text. Params `column`, `count`.
        ColumnHoldsText => "column_holds_text",
        /// A missing code equals a scale score's value in the column. Params
        /// `column`, `count`.
        CodeEqualsScore => "code_equals_score",
        /// The model is not identified, from the spec alone (decision 798):
        /// param `count`, the constraints it lacks, its parameters less the
        /// rank of its information at two points drawn from the spec.
        ModelNotIdentified => "model_not_identified",
        /// No row has a value in every column of the model.
        NoCompleteRows => "no_complete_rows",
        /// Fewer complete rows than columns plus one, so S cannot be positive
        /// definite. Params `count`, `limit`.
        TooFewRows => "too_few_rows",
        /// A column takes one value over the complete rows. Param `column`.
        ColumnConstant => "column_constant",
        /// A candidate missing code the rule found in a column and the user did
        /// not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// The run's refusal: S's Cholesky factor refuses at this column, which
        /// is a combination of the columns before it within the arithmetic.
        /// Param `column`.
        SampleCovarianceSingular => "sample_covariance_singular",
        /// The run's refusal: no attempt reached a solution (decision 798).
        /// Params `count` (the most iterations an attempt made) and `limit`
        /// (the largest cap among the attempts made).
        SemNotConverged => "sem_not_converged",
        /// The run's refusal: at the solution an estimate's √((H⁻¹)_aa) is
        /// above 10⁸ of its natural unit, so the data barely determine it
        /// (decision 798).
        SolutionNotIdentified => "solution_not_identified",
        /// Not computable: a fixed loading has no standard error, C.R. or P.
        FixedParameter => "fixed_parameter",
        /// Not computable: the model has 0 degrees of freedom. Also a note.
        DfZero => "df_zero",
        /// Not computable: HOELTER at a CMIN of 0.
        CminZero => "cmin_zero",
        /// Not computable: a comparison with the independence model whose value
        /// is not finite because the independence model's CMIN is 0 (decision
        /// 773).
        IndependenceCminZero => "independence_cmin_zero",
        /// Not computable: BCC and MECVI need more than p + 2 rows.
        TooFewRowsForBcc => "too_few_rows_for_bcc",
        /// Not computable: a square root meets a variance at or below 0. Also
        /// a note: a variance or error variance estimate below 0.
        NegativeVariance => "negative_variance",
        /// Not computable: a 0/0 of quantities that vanished (a NaN).
        TooSmall => "too_small",
        /// Not computable: a value along the way too large for a double. Also
        /// the run's refusal when a column's mean or an entry of S is not
        /// finite. Param `column`.
        Overflow => "overflow",
        /// Not computable: the distribution is not measured here.
        DistributionOutOfRange => "distribution_out_of_range",
        /// Note: the block of Φ̂ over the factors and every variable a
        /// covariance names is not positive definite.
        CovarianceNotPositiveDefinite => "covariance_not_positive_definite",
        /// Note: the paths hold a loop.
        Nonrecursive => "nonrecursive",
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
        /// Limitation: each column measures at most one factor, and a factor's
        /// columns are no path's end.
        IndicatorsMeasureOneFactor => "indicators_measure_one_factor",
        /// Limitation: a group held equal holds one kind of parameter.
        EqualOneKind => "equal_one_kind",
        /// Limitation: each factor's scale is its first column's loading of 1.
        MarkerIdentificationOnly => "marker_identification_only",
        /// Limitation: no parameter is fixed at a value of the user's.
        NoFixedValues => "no_fixed_values",
        /// Limitation: effects are listed among the structural variables only.
        EffectsStructuralOnly => "effects_structural_only",
        /// Limitation: no standard errors or intervals for indirect effects.
        NoIndirectEffectTests => "no_indirect_effect_tests",
        /// Limitation: no stability index for a loop.
        NoStabilityIndex => "no_stability_index",
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

/// A variable of the model: a factor by its number from 1, or a column by its
/// alias. Factors come first, by number, then columns by alias (decision 797).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemVariable {
    Factor(u32),
    Column(Alias),
}

/// A single-headed arrow from `from` to `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemPath {
    pub to: SemVariable,
    pub from: SemVariable,
}

/// A double-headed arrow between `a` and `b`, between their error terms where
/// they are endogenous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemCovariance {
    pub a: SemVariable,
    pub b: SemVariable,
}

/// A loading, by its factor's number and its column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemLoading {
    pub factor: u32,
    pub column: Alias,
}

/// A variance: of an exogenous variable, or of an endogenous one's error term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemVarianceOf {
    pub variable: SemVariable,
}

/// One member of a group held equal, ordered loading, path, covariance,
/// variance, then by its fields (decision 797).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemParameter {
    Loading(SemLoading),
    Path(SemPath),
    Covariance(SemCovariance),
    Variance(SemVarianceOf),
}

impl SemParameter {
    fn kind(&self) -> u8 {
        match self {
            SemParameter::Loading(_) => 0,
            SemParameter::Path(_) => 1,
            SemParameter::Covariance(_) => 2,
            SemParameter::Variance(_) => 3,
        }
    }
}

/// The structural equation model's parameters (decision 797): the factors, each
/// a list of two or more columns in the dataset's order, the factors in the
/// order of their first columns; the paths ascending by (to, from); the
/// covariances, each with a before b, ascending by (a, b); the groups held
/// equal, each ascending, the groups by their first members; and the missing
/// codes ascending, which apply to every column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemParams {
    pub indicators: Vec<Vec<Alias>>,
    pub paths: Vec<SemPath>,
    pub covariances: Vec<SemCovariance>,
    pub equal: Vec<Vec<SemParameter>>,
    pub missing_codes: Vec<f64>,
}

impl SemParams {
    /// Every column the model names, once each, in alias order: the factors'
    /// columns and the columns at a path's or covariance's end.
    pub fn columns(&self) -> Vec<Alias> {
        let mut out: BTreeSet<Alias> = self.indicators.iter().flatten().copied().collect();
        let ends = self.paths.iter().flat_map(|p| [p.to, p.from]).chain(self.covariances.iter().flat_map(|c| [c.a, c.b]));
        for v in ends {
            if let SemVariable::Column(a) = v {
                out.insert(a);
            }
        }
        out.into_iter().collect()
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
    NotComputable(SemCode),
}

/// What a regression weight is: a loading, from a factor to its column, or a
/// path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemWeightKind {
    Loading,
    Path,
}

/// One single-headed arrow's weight (AMOS's "Regression Weights"), with its
/// standard error, C.R. and P, and its standardized weight.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemWeight {
    pub to: SemVariable,
    pub from: SemVariable,
    pub kind: SemWeightKind,
    pub estimate: Stat,
    pub se: Stat,
    pub cr: Stat,
    pub p: Stat,
    pub standardized: Stat,
}

/// One double-headed arrow's covariance and its correlation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemCovarianceEstimate {
    pub a: SemVariable,
    pub b: SemVariable,
    pub estimate: Stat,
    pub se: Stat,
    pub cr: Stat,
    pub p: Stat,
    pub correlation: Stat,
}

/// One variable's variance, or its error term's (`error`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemVariance {
    pub variable: SemVariable,
    pub error: bool,
    pub estimate: Stat,
    pub se: Stat,
    pub cr: Stat,
    pub p: Stat,
}

/// One endogenous variable's squared multiple correlation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemSmc {
    pub variable: SemVariable,
    pub value: Stat,
}

/// The total, direct and indirect effects of `from` on `to`, and each
/// standardized.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemEffect {
    pub to: SemVariable,
    pub from: SemVariable,
    pub total: Stat,
    pub direct: Stat,
    pub indirect: Stat,
    pub total_standardized: Stat,
    pub direct_standardized: Stat,
    pub indirect_standardized: Stat,
}

/// AMOS's model-fit tables, each with three rows (decision 796).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemFit {
    pub model: SemModelFit,
    pub saturated: SemModelFit,
    pub independence: SemModelFit,
}

/// One model's row of AMOS's measures of fit (its Appendix C). The 90%
/// intervals are `…Lo90` and `…Hi90`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemModelFit {
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
}

/// The structural equation model's result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemOutcome {
    /// The model's columns, in alias order.
    pub columns: Vec<Alias>,
    pub rows_used: u64,
    /// The factors' columns, as the spec.
    pub indicators: Vec<Vec<Alias>>,
    /// Every single-headed arrow: the loadings, factor by factor, the fixed
    /// first ones included, then the paths in the spec's order.
    pub weights: Vec<SemWeight>,
    /// The spec's covariances, in its order.
    pub covariances: Vec<SemCovarianceEstimate>,
    /// Every variable's variance, or its error term's, in the model's order.
    pub variances: Vec<SemVariance>,
    /// Every endogenous variable's, in the model's order.
    pub smc: Vec<SemSmc>,
    /// Among the structural variables, each pair joined by a directed path.
    pub effects: Vec<SemEffect>,
    pub fit: SemFit,
    /// The steps taken by the attempt that reached the solution.
    pub iterations: u64,
    /// Which attempt reached it: 1, 2 or 3 (decision 798).
    pub attempt: u32,
    /// The rows left out, by column and reason; none with no row is listed.
    pub exclusions: Vec<Exclusion<SemCode>>,
    pub notes: Vec<SemCode>,
    pub limitations: Vec<SemCode>,
}

impl SemOutcome {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, closed words and aliases serialises")
    }
}

/// What every result says it does not give, in decision 799's order.
const LIMITATIONS: [SemCode; 15] = [
    SemCode::MlOnly,
    SemCode::NoMeans,
    SemCode::OneGroupOnly,
    SemCode::IndicatorsMeasureOneFactor,
    SemCode::EqualOneKind,
    SemCode::MarkerIdentificationOnly,
    SemCode::NoFixedValues,
    SemCode::EffectsStructuralOnly,
    SemCode::NoIndirectEffectTests,
    SemCode::NoStabilityIndex,
    SemCode::NoModificationIndices,
    SemCode::NormalTheoryStandardErrors,
    SemCode::NoSrmr,
    SemCode::ListwiseOnly,
    SemCode::NoWeights,
];

/* ------------------------------------------------------------------------ *
 *  The one reading of the rows
 * ------------------------------------------------------------------------ */

/// The complete rows' values, one vector per model column in alias order, and
/// each column's rows left out. Never leaves this module, and derives nothing.
struct Rows {
    values: Vec<Vec<f64>>,
    /// For each column: rows left out as `missing_value`, as `missing_code`.
    excluded: Vec<[u64; 2]>,
}

impl Rows {
    /// Listwise: a row is used only when every column holds a number that is
    /// not a code; a row left out is counted once, under its first column in
    /// alias order without one (decision 797). `columns` are in alias order.
    fn of(columns: &[&[Cell]], codes: &[f64]) -> Rows {
        let rows = columns.first().map_or(0, |c| c.len());
        let mut out = Rows {
            values: vec![Vec::with_capacity(rows); columns.len()],
            excluded: vec![[0; 2]; columns.len()],
        };
        let mut row = vec![0.0; columns.len()];
        'rows: for r in 0..rows {
            for (j, cells) in columns.iter().enumerate() {
                match &cells[r] {
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
 *  The model
 * ------------------------------------------------------------------------ */

/// One entry of A or Φ a parameter sets: A[t][s], or Φ[a][b] with a ≤ b (a
/// diagonal when a = b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    A(usize, usize),
    Phi(usize, usize),
}

/// A parameter's kind, for its starts and its identification draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Loading,
    Path,
    Covariance,
    Variance,
}

/// The model's shape over m = k + p variables in the model's order (the factors
/// by number, then the columns by alias), and θ's layout (decision 798).
#[derive(Debug, Clone)]
struct Model {
    k: usize,
    p: usize,
    m: usize,
    columns: Vec<Alias>,
    /// Each factor's columns, as variable indices, in alias order.
    factor_columns: Vec<Vec<usize>>,
    endogenous: Vec<bool>,
    /// Each column variable that measures a factor.
    indicator: Vec<bool>,
    /// Each parameter of θ: its entries, and its kind.
    params: Vec<Vec<Entry>>,
    kinds: Vec<Kind>,
    /// The first loadings, fixed at 1: (t, s).
    fixed: Vec<(usize, usize)>,
    /// The weights' rows: (to, from, kind, θ's index or none for a fixed one).
    weights: Vec<(usize, usize, SemWeightKind, Option<usize>)>,
    /// The covariances' rows: (a, b, θ's index), in the spec's order.
    covariances: Vec<(usize, usize, usize)>,
    /// Each variable's variance's θ index.
    variances: Vec<usize>,
    /// For each parameter, the most covariances at any end of any of its
    /// members (the identification draws' c).
    covariance_degree: Vec<usize>,
    /// Paths as (to, from) variable indices, for the effects and the loop.
    paths: Vec<(usize, usize)>,
    /// Each variable's natural unit once S is known ([`Model::in_units_of`]): the
    /// power of two at or below its representative column's √s (a factor's is
    /// its first column's), so scaling by it and back is exact; 1 before, so
    /// the identification draws, which are in no unit, keep their bits. For a
    /// model with a loop, `implied` inverts I − A in these units.
    scale: Vec<f64>,
    /// Each variable's sources: the variables with an arrow into it (a loading
    /// or a path), ascending.
    sources: Vec<Vec<usize>>,
    /// The variables in an order where every arrow goes forward, the lowest
    /// index first among those free to come next; `None` when the arrows hold a
    /// loop.
    order: Option<Vec<usize>>,
}

impl Model {
    fn variable(&self, v: SemVariable) -> usize {
        match v {
            SemVariable::Factor(n) => n as usize - 1,
            SemVariable::Column(a) => self.k + self.columns.iter().position(|c| *c == a).expect("a model column"),
        }
    }

    fn name(&self, v: usize) -> SemVariable {
        if v < self.k {
            SemVariable::Factor(v as u32 + 1)
        } else {
            SemVariable::Column(self.columns[v - self.k])
        }
    }

    /// The model of a spec the checks passed (decision 798's layout).
    fn of(params: &SemParams) -> Model {
        let k = params.indicators.len();
        let columns = params.columns();
        let p = columns.len();
        let m = k + p;
        let mut model = Model {
            k,
            p,
            m,
            columns,
            factor_columns: Vec::with_capacity(k),
            endogenous: vec![false; m],
            indicator: vec![false; m],
            params: Vec::new(),
            kinds: Vec::new(),
            fixed: Vec::new(),
            weights: Vec::new(),
            covariances: Vec::new(),
            variances: Vec::new(),
            covariance_degree: Vec::new(),
            paths: Vec::new(),
            scale: vec![1.0; m],
            sources: vec![Vec::new(); m],
            order: None,
        };
        // The raw parameters in θ's layout, each with the spec's name for it.
        let mut raw: Vec<(SemParameter, Entry, Kind)> = Vec::new();
        for (f, cols) in params.indicators.iter().enumerate() {
            let vars: Vec<usize> = cols.iter().map(|a| model.variable(SemVariable::Column(*a))).collect();
            for (t, v) in vars.iter().enumerate() {
                model.endogenous[*v] = true;
                model.indicator[*v] = true;
                if t == 0 {
                    model.fixed.push((*v, f));
                } else {
                    raw.push((SemParameter::Loading(SemLoading { factor: f as u32 + 1, column: cols[t] }), Entry::A(*v, f), Kind::Loading));
                }
            }
            model.factor_columns.push(vars);
        }
        for path in &params.paths {
            let (t, s) = (model.variable(path.to), model.variable(path.from));
            model.endogenous[t] = true;
            model.paths.push((t, s));
            raw.push((SemParameter::Path(*path), Entry::A(t, s), Kind::Path));
        }
        for c in &params.covariances {
            let (a, b) = (model.variable(c.a), model.variable(c.b));
            raw.push((SemParameter::Covariance(*c), Entry::Phi(a.min(b), a.max(b)), Kind::Covariance));
        }
        for v in 0..m {
            raw.push((SemParameter::Variance(SemVarianceOf { variable: model.name(v) }), Entry::Phi(v, v), Kind::Variance));
        }
        // Each raw parameter's group, if any; a group's members share the
        // position of the member first in the layout.
        let group_of: BTreeMap<SemParameter, usize> =
            params.equal.iter().enumerate().flat_map(|(g, members)| members.iter().map(move |x| (*x, g))).collect();
        let mut group_index: BTreeMap<usize, usize> = BTreeMap::new();
        let mut position = Vec::with_capacity(raw.len());
        for (name, entry, kind) in &raw {
            let at = match group_of.get(name).and_then(|g| group_index.get(g)) {
                Some(&i) => i,
                None => {
                    let i = model.params.len();
                    model.params.push(Vec::new());
                    model.kinds.push(*kind);
                    if let Some(g) = group_of.get(name) {
                        group_index.insert(*g, i);
                    }
                    i
                }
            };
            model.params[at].push(*entry);
            position.push(at);
        }
        // The rows of the result, in decision 799's order.
        let mut r = 0;
        for (f, vars) in model.factor_columns.clone().iter().enumerate() {
            for (t, v) in vars.iter().enumerate() {
                if t == 0 {
                    model.weights.push((*v, f, SemWeightKind::Loading, None));
                } else {
                    model.weights.push((*v, f, SemWeightKind::Loading, Some(position[r])));
                    r += 1;
                }
            }
        }
        for (t, s) in model.paths.clone() {
            model.weights.push((t, s, SemWeightKind::Path, Some(position[r])));
            r += 1;
        }
        for c in &params.covariances {
            let (a, b) = (model.variable(c.a), model.variable(c.b));
            model.covariances.push((a, b, position[r]));
            r += 1;
        }
        for _ in 0..m {
            model.variances.push(position[r]);
            r += 1;
        }
        // The identification draws' c: the most covariances at any end of any
        // member.
        let mut at_end = vec![0usize; m];
        for (a, b, _) in &model.covariances {
            at_end[*a] += 1;
            at_end[*b] += 1;
        }
        model.covariance_degree = model
            .params
            .iter()
            .map(|entries| {
                entries
                    .iter()
                    .map(|e| match e {
                        Entry::Phi(a, b) if a != b => at_end[*a].max(at_end[*b]),
                        _ => 0,
                    })
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        for (t, from, _, _) in &model.weights {
            model.sources[*t].push(*from);
        }
        for list in &mut model.sources {
            list.sort_unstable();
            list.dedup();
        }
        model.order = topological(&model.sources);
        model
    }

    fn q(&self) -> usize {
        self.params.len()
    }

    /// A and Φ at θ.
    fn matrices(&self, theta: &[f64]) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
        let mut a = vec![vec![0.0; self.m]; self.m];
        let mut phi = vec![vec![0.0; self.m]; self.m];
        for (t, s) in &self.fixed {
            a[*t][*s] = 1.0;
        }
        for (x, entries) in self.params.iter().enumerate() {
            for e in entries {
                match *e {
                    Entry::A(t, s) => a[t][s] = theta[x],
                    Entry::Phi(i, j) => {
                        phi[i][j] = theta[x];
                        phi[j][i] = theta[x];
                    }
                }
            }
        }
        (a, phi)
    }

    /// Sets each variable's natural unit from S (see `scale`): √s with its
    /// mantissa's bits cleared, 2 to the power of its exponent, read from the
    /// bits so the same on every platform. S is positive definite here, so √s
    /// is a positive normal number.
    fn in_units_of(&mut self, s: &[Vec<f64>]) {
        let k = self.k;
        self.scale = (0..self.m)
            .map(|v| {
                let r = if v < k { self.factor_columns[v][0] - k } else { v - k };
                f64::from_bits(s[r][r].sqrt().to_bits() & 0x7FF0_0000_0000_0000)
            })
            .collect();
    }

    /// B = (I − A)⁻¹, Ω = B Φ Bᵀ and Σ, Ω's block over the columns; `None`
    /// when I − A cannot be inverted.
    ///
    /// Without a loop, B by substitution in `order`: B[t] = e_t + Σ A[t][s] B[s]
    /// over t's sources, with no pivot and no tolerance (I − A is then always
    /// invertible), and B[t][s] exactly A[t][s] where that arrow is the only
    /// route, so such an indirect effect is exactly 0. With a loop, Gauss–Jordan
    /// in natural units: I − D⁻¹ A D with D the units (powers of two, so
    /// exactly), and B is D (its inverse) D⁻¹.
    ///
    /// Before (9C-4's hunt, 9C-2's h226), I − A was inverted as it stands, and
    /// pivoting on a loading λ above 1 leaves a pivot near 1/λ, so `inverse`'s
    /// rule (a pivot at or below 10⁻¹² of the largest entry) refused every
    /// loading above about 10⁶: a column on another scale made F undefined
    /// (`a_column_on_another_scale_changes_only_its_own_weights`).
    fn implied(&self, theta: &[f64]) -> Option<Implied> {
        let (a, phi) = self.matrices(theta);
        let b = match &self.order {
            Some(order) => {
                let mut b = vec![vec![0.0; self.m]; self.m];
                for &t in order {
                    let mut row = vec![0.0; self.m];
                    row[t] = 1.0;
                    for &from in &self.sources[t] {
                        for (x, y) in row.iter_mut().zip(&b[from]) {
                            *x += a[t][from] * y;
                        }
                    }
                    b[t] = row;
                }
                b
            }
            None => {
                let d = &self.scale;
                let i_minus_a: Vec<Vec<f64>> = (0..self.m)
                    .map(|i| (0..self.m).map(|j| if i == j { 1.0 } else { 0.0 } - a[i][j] * d[j] / d[i]).collect())
                    .collect();
                let natural = inverse(&i_minus_a)?;
                (0..self.m).map(|i| (0..self.m).map(|j| natural[i][j] * d[i] / d[j]).collect()).collect()
            }
        };
        let bt: Vec<Vec<f64>> = (0..self.m).map(|i| (0..self.m).map(|j| b[j][i]).collect()).collect();
        let omega = multiply(&multiply(&b, &phi), &bt);
        let sigma: Vec<Vec<f64>> = (self.k..self.m).map(|i| (self.k..self.m).map(|j| omega[i][j]).collect()).collect();
        Some(Implied { a, phi, b, omega, sigma })
    }
}

/// A topological order of the variables by their sources (Kahn's, the lowest
/// index first among those whose sources have all come), or `None` when some
/// never can: the arrows hold a loop.
fn topological(sources: &[Vec<usize>]) -> Option<Vec<usize>> {
    let m = sources.len();
    let mut waiting: Vec<usize> = sources.iter().map(Vec::len).collect();
    let mut order = Vec::with_capacity(m);
    let mut placed = vec![false; m];
    while order.len() < m {
        let next = (0..m).find(|v| !placed[*v] && waiting[*v] == 0)?;
        placed[next] = true;
        order.push(next);
        for (t, list) in sources.iter().enumerate() {
            waiting[t] -= list.iter().filter(|s| **s == next).count();
        }
    }
    Some(order)
}

/// What θ implies.
struct Implied {
    a: Vec<Vec<f64>>,
    phi: Vec<Vec<f64>>,
    b: Vec<Vec<f64>>,
    omega: Vec<Vec<f64>>,
    sigma: Vec<Vec<f64>>,
}

/// A vector a Σ_a is built from: g_v, column v of G = F B, or h_v, column v of
/// K = F Ω.
#[derive(Clone, Copy)]
enum Piece {
    G(usize),
    K(usize),
}

/// One evaluation at θ: F, g, H, and J when asked (the Newton attempt's), with
/// Σ⁻¹.
struct Evaluation {
    f: f64,
    g: Vec<f64>,
    h: Vec<Vec<f64>>,
    j: Option<Vec<Vec<f64>>>,
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

/// Xᵀ Y for two p × m matrices.
fn cross(x: &[Vec<f64>], y: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let m = x.first().map_or(0, Vec::len);
    let n = y.first().map_or(0, Vec::len);
    let mut out = vec![vec![0.0; n]; m];
    for (rx, ry) in x.iter().zip(y) {
        for (u, xu) in rx.iter().enumerate() {
            for (v, yv) in ry.iter().enumerate() {
                out[u][v] += xu * yv;
            }
        }
    }
    out
}

/// The bilinear forms a parameter's pieces meet: Gᵀ M G, Gᵀ M K and Kᵀ M K for
/// one symmetric p × p matrix M.
struct Forms {
    gg: Vec<Vec<f64>>,
    gk: Vec<Vec<f64>>,
    kk: Vec<Vec<f64>>,
}

impl Forms {
    fn of(m: &[Vec<f64>], g: &[Vec<f64>], k: &[Vec<f64>]) -> Forms {
        let mg = multiply(m, g);
        let mk = multiply(m, k);
        Forms { gg: cross(g, &mg), gk: cross(g, &mk), kk: cross(k, &mk) }
    }

    /// xᵀ M y.
    fn at(&self, x: Piece, y: Piece) -> f64 {
        match (x, y) {
            (Piece::G(u), Piece::G(v)) => self.gg[u][v],
            (Piece::G(u), Piece::K(v)) => self.gk[u][v],
            (Piece::K(u), Piece::G(v)) => self.gk[v][u],
            (Piece::K(u), Piece::K(v)) => self.kk[u][v],
        }
    }
}

/// An entry's Σ_a as pieces x yᵀ: A[t][s] gives g_t h_sᵀ + h_s g_tᵀ; Φ[a][b], a
/// ≠ b, g_a g_bᵀ + g_b g_aᵀ; Φ[v][v], g_v g_vᵀ (decision 798).
fn pieces(e: Entry) -> Vec<(Piece, Piece)> {
    match e {
        Entry::A(t, s) => vec![(Piece::G(t), Piece::K(s)), (Piece::K(s), Piece::G(t))],
        Entry::Phi(a, b) if a != b => vec![(Piece::G(a), Piece::G(b)), (Piece::G(b), Piece::G(a))],
        Entry::Phi(v, _) => vec![(Piece::G(v), Piece::G(v))],
    }
}

/// Σ over the pieces x yᵀ of a and z wᵀ of b of (yᵀ M z)(wᵀ N x).
fn piece_sum(a: &[(Piece, Piece)], b: &[(Piece, Piece)], m: &Forms, n: &Forms) -> f64 {
    let mut v = 0.0;
    for (x, y) in a {
        for (z, w) in b {
            v += m.at(*y, *z) * n.at(*w, *x);
        }
    }
    v
}

/// H_ab = tr(Σ⁻¹ Σ_a Σ⁻¹ Σ_b) over the parameters' pieces, given the forms of
/// Σ⁻¹ (decision 798).
fn information(model: &Model, inv: &Forms) -> Vec<Vec<f64>> {
    let q = model.q();
    let pieces_of: Vec<Vec<(Piece, Piece)>> = model.params.iter().map(|es| es.iter().flat_map(|e| pieces(*e)).collect()).collect();
    let mut h = vec![vec![0.0; q]; q];
    for x in 0..q {
        for y in 0..=x {
            let v = piece_sum(&pieces_of[x], &pieces_of[y], inv, inv);
            h[x][y] = v;
            h[y][x] = v;
        }
    }
    h
}

/// G and K, the column variables' rows of B and Ω (p × m).
fn g_and_k(model: &Model, imp: &Implied) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    (imp.b[model.k..].to_vec(), imp.omega[model.k..].to_vec())
}

/// F at θ, or `None` when I − A cannot be inverted or Σ(θ) is not positive
/// definite.
fn discrepancy(model: &Model, s: &[Vec<f64>], log_det_s: f64, theta: &[f64]) -> Option<f64> {
    let imp = model.implied(theta)?;
    let l = cholesky_within(&imp.sigma, 0.0).ok()?;
    let inv = spd_inverse(&l);
    Some(log_det(&l) + trace_product(s, &inv) - log_det_s - model.p as f64)
}

/// F, g, H and, with `observed`, J at θ (decision 798), or `None` when I − A
/// cannot be inverted or Σ(θ) is not positive definite.
fn evaluate(model: &Model, s: &[Vec<f64>], log_det_s: f64, theta: &[f64], observed: bool) -> Option<Evaluation> {
    let p = model.p;
    let imp = model.implied(theta)?;
    let l = cholesky_within(&imp.sigma, 0.0).ok()?;
    let ai = spd_inverse(&l);
    let f = log_det(&l) + trace_product(s, &ai) - log_det_s - p as f64;
    let (gm, km) = g_and_k(model, &imp);
    // R = Σ⁻¹ − Σ⁻¹ S Σ⁻¹; P = Σ⁻¹ S Σ⁻¹.
    let pm = multiply(&multiply(&ai, s), &ai);
    let r: Vec<Vec<f64>> = (0..p).map(|i| (0..p).map(|j| ai[i][j] - pm[i][j]).collect()).collect();
    let rg = multiply(&r, &gm);
    let grg = cross(&gm, &rg);
    let krg = cross(&km, &rg);
    let entry_gradient = |e: Entry| -> f64 {
        match e {
            Entry::A(t, s) => 2.0 * krg[s][t],
            Entry::Phi(a, b) if a != b => 2.0 * grg[a][b],
            Entry::Phi(v, _) => grg[v][v],
        }
    };
    let g: Vec<f64> = model.params.iter().map(|es| es.iter().map(|e| entry_gradient(*e)).sum()).collect();
    let inv_forms = Forms::of(&ai, &gm, &km);
    let h = information(model, &inv_forms);
    let j = if observed {
        let p_forms = Forms::of(&pm, &gm, &km);
        Some(observed_information(model, &imp, &h, &inv_forms, &p_forms, &grg, &krg))
    } else {
        None
    };
    Some(Evaluation { f, g, h, j })
}

/// J_ab = −H_ab + 2 T_ab + U_ab (decision 798): T_ab = tr(Σ⁻¹ Σ_a P Σ_b) over the
/// pieces, U_ab = tr(R Σ_ab) over the entry pairs, from G'RG and K'RG.
fn observed_information(
    model: &Model,
    imp: &Implied,
    h: &[Vec<f64>],
    inv: &Forms,
    pf: &Forms,
    grg: &[Vec<f64>],
    krg: &[Vec<f64>],
) -> Vec<Vec<f64>> {
    let q = model.q();
    let b = &imp.b;
    let omega = &imp.omega;
    let u = |e1: Entry, e2: Entry| -> f64 {
        match (e1, e2) {
            (Entry::A(i, j), Entry::A(k, l)) => 2.0 * (b[l][i] * krg[j][k] + omega[l][j] * grg[i][k] + b[j][k] * krg[l][i]),
            (Entry::A(i, j), Entry::Phi(k, l)) | (Entry::Phi(k, l), Entry::A(i, j)) => {
                if k == l {
                    2.0 * b[j][k] * grg[k][i]
                } else {
                    2.0 * (b[j][l] * grg[k][i] + b[j][k] * grg[l][i])
                }
            }
            (Entry::Phi(..), Entry::Phi(..)) => 0.0,
        }
    };
    let pieces_of: Vec<Vec<(Piece, Piece)>> = model.params.iter().map(|es| es.iter().flat_map(|e| pieces(*e)).collect()).collect();
    let mut j = vec![vec![0.0; q]; q];
    for x in 0..q {
        for y in 0..=x {
            let t = piece_sum(&pieces_of[x], &pieces_of[y], pf, inv);
            let mut us = 0.0;
            for e1 in &model.params[x] {
                for e2 in &model.params[y] {
                    us += u(*e1, *e2);
                }
            }
            let v = -h[x][y] + 2.0 * t + us;
            j[x][y] = v;
            j[y][x] = v;
        }
    }
    j
}

/* ------------------------------------------------------------------------ *
 *  validate
 * ------------------------------------------------------------------------ */

/// The capability's id, parsed.
pub fn capability_id() -> CapabilityId {
    CapabilityId::parse(CAPABILITY_ID).expect("the capability id is well formed")
}

/// A spec for this capability on this dataset.
pub fn spec(operation_id: OperationId, dataset: &Dataset, parameters: SemParams) -> OperationSpec<SemParams> {
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

fn column_param(alias: Alias) -> ParamValue<SemCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

struct Checked<'d> {
    errors: Vec<Diagnostic<SemCode>>,
    ready: Option<(Vec<&'d [Cell]>, Model)>,
}

/// The checks of the model's shape, its paths, its covariances and its groups
/// held equal, in decision 797's order, each family reporting every error it
/// finds.
fn check_spec(params: &SemParams, errors: &mut Vec<Diagnostic<SemCode>>) {
    let k = params.indicators.len();
    // The model's shape.
    if k == 0 && params.paths.is_empty() && params.covariances.is_empty() {
        errors.push(Diagnostic::error(SemCode::EmptyModel));
    }
    if k > MAX_FACTORS {
        errors.push(
            Diagnostic::error(SemCode::TooManyFactors)
                .int(ParamKey::Count, k as i64)
                .int(ParamKey::Limit, MAX_FACTORS as i64),
        );
    }
    for (f, columns) in params.indicators.iter().enumerate() {
        if columns.len() < MIN_FACTOR_ITEMS {
            errors.push(
                Diagnostic::error(SemCode::FactorTooFewItems)
                    .int(ParamKey::Index, f as i64 + 1)
                    .int(ParamKey::Count, columns.len() as i64)
                    .int(ParamKey::Limit, MIN_FACTOR_ITEMS as i64),
            );
        }
    }
    for columns in &params.indicators {
        for w in columns.windows(2) {
            if w[0] > w[1] {
                errors.push(Diagnostic::error(SemCode::ItemsOutOfOrder).with(ParamKey::Column, column_param(w[1])));
            }
        }
    }
    for (f, w) in params.indicators.windows(2).enumerate() {
        if let (Some(a), Some(b)) = (w[0].first(), w[1].first()) {
            if a > b {
                errors.push(Diagnostic::error(SemCode::FactorsOutOfOrder).int(ParamKey::Index, f as i64 + 2));
            }
        }
    }
    let mut seen = BTreeSet::new();
    for alias in params.indicators.iter().flatten() {
        if !seen.insert(*alias) {
            errors.push(Diagnostic::error(SemCode::ItemRepeated).with(ParamKey::Column, column_param(*alias)));
        }
    }
    let indicator: BTreeSet<Alias> = params.indicators.iter().flatten().copied().collect();
    let known = |v: SemVariable| match v {
        SemVariable::Factor(n) => n >= 1 && n as usize <= k,
        SemVariable::Column(_) => true,
    };
    // The paths.
    if params.paths.len() > MAX_PATHS {
        errors.push(
            Diagnostic::error(SemCode::TooManyPaths)
                .int(ParamKey::Count, params.paths.len() as i64)
                .int(ParamKey::Limit, MAX_PATHS as i64),
        );
    }
    let mut seen_paths = BTreeSet::new();
    for (i, path) in params.paths.iter().enumerate() {
        let index = i as i64 + 1;
        if !known(path.to) || !known(path.from) {
            errors.push(Diagnostic::error(SemCode::UnknownFactor).int(ParamKey::Index, index));
        }
        if path.to == path.from {
            errors.push(Diagnostic::error(SemCode::PathToItself).int(ParamKey::Index, index));
        }
        for end in [path.to, path.from] {
            if let SemVariable::Column(a) = end {
                if indicator.contains(&a) {
                    errors.push(Diagnostic::error(SemCode::IndicatorInPath).with(ParamKey::Column, column_param(a)));
                }
            }
        }
        if i > 0 && params.paths[i - 1] > *path {
            errors.push(Diagnostic::error(SemCode::PathsOutOfOrder).int(ParamKey::Index, index));
        }
        if !seen_paths.insert(*path) {
            errors.push(Diagnostic::error(SemCode::PathRepeated).int(ParamKey::Index, index));
        }
    }
    // The covariances.
    if params.covariances.len() > MAX_COVARIANCES {
        errors.push(
            Diagnostic::error(SemCode::TooManyCovariances)
                .int(ParamKey::Count, params.covariances.len() as i64)
                .int(ParamKey::Limit, MAX_COVARIANCES as i64),
        );
    }
    let mut seen_covariances = BTreeSet::new();
    for (i, c) in params.covariances.iter().enumerate() {
        let index = i as i64 + 1;
        if !known(c.a) || !known(c.b) {
            errors.push(Diagnostic::error(SemCode::UnknownFactor).int(ParamKey::Index, index));
        }
        if c.a == c.b {
            errors.push(Diagnostic::error(SemCode::CovarianceWithItself).int(ParamKey::Index, index));
        }
        if c.a > c.b || (i > 0 && params.covariances[i - 1] > *c) {
            errors.push(Diagnostic::error(SemCode::CovariancesOutOfOrder).int(ParamKey::Index, index));
        }
        if !seen_covariances.insert(*c) {
            errors.push(Diagnostic::error(SemCode::CovarianceRepeated).int(ParamKey::Index, index));
        }
    }
    // The groups held equal.
    let members: usize = params.equal.iter().map(Vec::len).sum();
    if members > MAX_EQUAL_MEMBERS {
        errors.push(
            Diagnostic::error(SemCode::TooManyEqualMembers)
                .int(ParamKey::Count, members as i64)
                .int(ParamKey::Limit, MAX_EQUAL_MEMBERS as i64),
        );
    }
    let columns: BTreeSet<Alias> = params.columns().into_iter().collect();
    let has_variable = |v: SemVariable| match v {
        SemVariable::Factor(n) => n >= 1 && n as usize <= k,
        SemVariable::Column(a) => columns.contains(&a),
    };
    let mut held = BTreeSet::new();
    for (g, group) in params.equal.iter().enumerate() {
        let index = g as i64 + 1;
        if group.len() < MIN_EQUAL_MEMBERS {
            errors.push(
                Diagnostic::error(SemCode::EqualGroupTooSmall)
                    .int(ParamKey::Index, index)
                    .int(ParamKey::Count, group.len() as i64)
                    .int(ParamKey::Limit, MIN_EQUAL_MEMBERS as i64),
            );
        }
        if group.windows(2).any(|w| w[0].kind() != w[1].kind()) {
            errors.push(Diagnostic::error(SemCode::EqualMixedKinds).int(ParamKey::Index, index));
        }
        for (t, member) in group.iter().enumerate() {
            let at = |code: SemCode| Diagnostic::error(code).int(ParamKey::Index, index).int(ParamKey::Value, t as i64 + 1);
            match member {
                SemParameter::Loading(l) => {
                    let factor = (l.factor >= 1 && l.factor as usize <= k).then(|| &params.indicators[l.factor as usize - 1]);
                    match factor {
                        Some(cols) if cols.first() == Some(&l.column) => errors.push(at(SemCode::EqualFixedParameter)),
                        Some(cols) if cols.contains(&l.column) => {}
                        _ => errors.push(at(SemCode::EqualUnknownParameter)),
                    }
                }
                SemParameter::Path(path) => {
                    if !params.paths.contains(path) {
                        errors.push(at(SemCode::EqualUnknownParameter));
                    }
                }
                SemParameter::Covariance(c) => {
                    if !params.covariances.contains(c) {
                        errors.push(at(SemCode::EqualUnknownParameter));
                    }
                }
                SemParameter::Variance(v) => {
                    if !has_variable(v.variable) {
                        errors.push(at(SemCode::EqualUnknownParameter));
                    }
                }
            }
        }
        if g > 0 {
            if let (Some(before), Some(first)) = (params.equal[g - 1].first(), group.first()) {
                if before > first {
                    errors.push(Diagnostic::error(SemCode::EqualOutOfOrder).int(ParamKey::Index, index));
                }
            }
        }
        for (t, w) in group.windows(2).enumerate() {
            if w[0] > w[1] {
                errors.push(Diagnostic::error(SemCode::EqualOutOfOrder).int(ParamKey::Index, index).int(ParamKey::Value, t as i64 + 2));
            }
        }
        for (t, member) in group.iter().enumerate() {
            if !held.insert(*member) {
                errors.push(Diagnostic::error(SemCode::EqualRepeated).int(ParamKey::Index, index).int(ParamKey::Value, t as i64 + 1));
            }
        }
    }
}

/// Shared by `validate` and `run` (decision 797's order): the missing codes;
/// the model's shape, paths, covariances and groups held equal; the columns'
/// count and each column; then, only when there is no error, the model's
/// identification from the spec alone; then, only when there is no other
/// error, the first of: no complete row, too few rows, a column constant.
fn check<'d>(dataset: &'d Dataset, params: &SemParams) -> Checked<'d> {
    let mut errors = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(Diagnostic::error(SemCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64));
        }
    }
    for (i, w) in params.missing_codes.windows(2).enumerate() {
        if w[0] == w[1] {
            errors.push(Diagnostic::error(SemCode::MissingCodeRepeated).int(ParamKey::Index, i as i64 + 1));
        } else if w[0] > w[1] {
            errors.push(Diagnostic::error(SemCode::MissingCodesOutOfOrder).int(ParamKey::Index, i as i64 + 1));
        }
    }
    check_spec(params, &mut errors);
    let columns = params.columns();
    let p = columns.len();
    if p < MIN_COLUMNS {
        errors.push(
            Diagnostic::error(SemCode::TooFewColumns)
                .int(ParamKey::Count, p as i64)
                .int(ParamKey::Limit, MIN_COLUMNS as i64),
        );
    } else if p > MAX_COLUMNS {
        errors.push(
            Diagnostic::error(SemCode::TooManyColumns)
                .int(ParamKey::Count, p as i64)
                .int(ParamKey::Limit, MAX_COLUMNS as i64),
        );
    }
    let mut cells = Vec::with_capacity(p);
    for alias in &columns {
        let Some((summary, column_cells)) = column(dataset, *alias) else {
            errors.push(Diagnostic::error(SemCode::UnknownColumn).with(ParamKey::Column, column_param(*alias)));
            continue;
        };
        let non_finite = non_finite_cells(summary);
        let texts = column_cells.iter().filter(|c| matches!(c, Cell::Text(_))).count() as i64;
        if non_finite > 0 {
            errors.push(
                Diagnostic::error(SemCode::ColumnNonFinite)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, non_finite as i64),
            );
        } else if texts > 0 {
            errors.push(
                Diagnostic::error(SemCode::ColumnHoldsText)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, texts),
            );
        }
        cells.push(column_cells);
    }
    for alias in &columns {
        let collisions = score_code_count(dataset, *alias, &params.missing_codes);
        if collisions > 0 {
            errors.push(
                Diagnostic::error(SemCode::CodeEqualsScore)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, collisions as i64),
            );
        }
    }
    if !errors.is_empty() {
        return Checked { errors, ready: None };
    }
    let model = Model::of(params);
    if let Err(count) = identified(&model) {
        errors.push(Diagnostic::error(SemCode::ModelNotIdentified).int(ParamKey::Count, count as i64));
        return Checked { errors, ready: None };
    }
    let rows = Rows::of(&cells, &params.missing_codes);
    let n = rows.complete();
    if n == 0 {
        errors.push(Diagnostic::error(SemCode::NoCompleteRows));
    } else if n < p + 1 {
        errors.push(
            Diagnostic::error(SemCode::TooFewRows)
                .int(ParamKey::Count, n as i64)
                .int(ParamKey::Limit, (p + 1) as i64),
        );
    } else if let Some(j) = rows.values.iter().position(|x| constant(x)) {
        errors.push(Diagnostic::error(SemCode::ColumnConstant).with(ParamKey::Column, column_param(columns[j])));
    }
    Checked { errors, ready: Some((cells, model)) }
}

/// Checks the spec against the dataset. Unsupported when it names another
/// capability, version or workspace; invalid on a wrong input or a parameter
/// error; a warning for each candidate missing code in a column that the user
/// did not list, a score's skipped. Nothing needs input.
pub fn validate(dataset: &Dataset, spec: &OperationSpec<SemParams>) -> ValidationReport<SemCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID {
        unsupported.push(Diagnostic::error(SemCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(SemCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(SemCode::InputMismatch));
            }
        }
        inputs => errors.push(Diagnostic::error(SemCode::InputCount).int(ParamKey::Count, inputs.len() as i64)),
    }
    if !preparation_holds(dataset, spec.preparation.as_ref()) {
        errors.push(Diagnostic::error(SemCode::PreparationMismatch));
    }
    let params = &spec.parameters;
    errors.extend(check(dataset, params).errors);
    let mut warnings = Vec::new();
    for alias in params.columns() {
        if let Some((_, cells)) = column(dataset, alias) {
            for s in suspect_codes_unless_score(dataset, alias, cells) {
                if !params.missing_codes.contains(&f64::from(s.code)) {
                    warnings.push(
                        Diagnostic::warning(SemCode::SuspectMissingCode)
                            .with(ParamKey::Column, column_param(alias))
                            .int(ParamKey::Value, i64::from(s.code))
                            .int(ParamKey::Count, s.cells as i64),
                    );
                }
            }
        }
    }
    let rows = dataset.rows() as u64;
    let columns = params.columns().len() as u64;
    let estimate = ResourceEstimate {
        rows,
        columns,
        cells: rows.saturating_mul(columns),
    };
    ValidationReport::new(unsupported, errors, Vec::new(), warnings, estimate)
}

/* ------------------------------------------------------------------------ *
 *  Identification from the spec alone
 * ------------------------------------------------------------------------ */

/// SplitMix64, the draws' generator (decision 798).
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// u uniform on [0, 1) from the top 53 bits.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / 9_007_199_254_740_992.0
    }
}

/// The identification check's point from `seed`: one draw per parameter in θ's
/// order (decision 798): a loading 0.5 + u, a path u − 0.5, a variance 1 + u, a
/// covariance (u − 0.5) / (1 + c).
fn draw(model: &Model, seed: u64) -> Vec<f64> {
    let mut g = SplitMix(seed);
    model
        .kinds
        .iter()
        .zip(&model.covariance_degree)
        .map(|(kind, c)| {
            let u = g.unit();
            match kind {
                Kind::Loading => 0.5 + u,
                Kind::Path => u - 0.5,
                Kind::Variance => 1.0 + u,
                Kind::Covariance => (u - 0.5) / (1.0 + *c as f64),
            }
        })
        .collect()
}

/// H at θ scaled to a unit diagonal, a parameter whose diagonal is 0 left out (a
/// lacking constraint); `None` when I − A cannot be inverted or Σ is not positive
/// definite there.
fn scaled_information(model: &Model, theta: &[f64]) -> Option<Vec<Vec<f64>>> {
    let imp = model.implied(theta)?;
    let l = cholesky_within(&imp.sigma, 0.0).ok()?;
    let ai = spd_inverse(&l);
    let (gm, km) = g_and_k(model, &imp);
    let h = information(model, &Forms::of(&ai, &gm, &km));
    let kept: Vec<usize> = (0..h.len()).filter(|a| h[*a][*a] > 0.0).collect();
    let scale: Vec<f64> = kept.iter().map(|a| 1.0 / h[*a][*a].sqrt()).collect();
    let scaled: Vec<Vec<f64>> =
        kept.iter().enumerate().map(|(x, a)| kept.iter().enumerate().map(|(y, b)| h[*a][*b] * scale[x] * scale[y]).collect()).collect();
    Some(scaled)
}

/// The rank of the scaled information at θ by [`pivoted_rank`] at
/// [`IDENTIFICATION_TOLERANCE`]; 0 where [`scaled_information`] gives none.
fn rank_at(model: &Model, theta: &[f64]) -> usize {
    scaled_information(model, theta).map_or(0, |h| pivoted_rank(&h, IDENTIFICATION_TOLERANCE))
}

/// Decision 798: the larger rank of H at the two points; `Err` with the count
/// of constraints the model lacks, q less that rank, when it is below q.
fn identified(model: &Model) -> Result<(), usize> {
    let q = model.q();
    let rank = IDENTIFICATION_SEEDS.iter().map(|seed| rank_at(model, &draw(model, *seed))).max().unwrap_or(0);
    if rank < q {
        Err(q - rank)
    } else {
        Ok(())
    }
}

/* ------------------------------------------------------------------------ *
 *  run
 * ------------------------------------------------------------------------ */

/// Why [`run`] returned no result.
#[derive(Debug, Clone, PartialEq)]
pub enum Halt<B> {
    /// The parameters are wrong for this dataset, or the run's own refusal.
    Refused(Diagnostic<SemCode>),
    /// The caller's check point stopped the run.
    Stopped { reason: B },
}

/// How many check points a run of `params` calls (decision 799): one before
/// the arithmetic, one before each column of S, one before the estimation, one
/// before the estimates and effects, one before the default model's fit and one
/// before the independence model's. The estimation's comes again at every
/// iteration of every attempt, every Newton solve and the check at the
/// solution; each fit row's at each step of its interval searches.
pub fn check_points(params: &SemParams) -> usize {
    1 + params.columns().len() + 4
}

/// The run as a function of the data alone, every check point continuing.
pub fn compute(dataset: &Dataset, params: &SemParams) -> Result<SemOutcome, Diagnostic<SemCode>> {
    compute_with_caps(dataset, params, [MAX_ITERATIONS; 3])
}

/// [`compute`] with each attempt's cap on iterations as an argument (decision
/// 800), so a test can end an attempt at its cap and reach the next.
pub fn compute_with_caps(dataset: &Dataset, params: &SemParams, caps: [u32; 3]) -> Result<SemOutcome, Diagnostic<SemCode>> {
    match run_with_caps(dataset, params, caps, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The structural equation model. `checkpoint(0)` is called after the
/// parameters are checked and before the arithmetic, `checkpoint(1..=p)`
/// before each column of S, `checkpoint(p + 1)` before the estimation and
/// again at every iteration and Newton solve of every attempt and before the
/// check at the solution, `checkpoint(p + 2)` before the estimates and
/// effects, `checkpoint(p + 3)` before the default model's fit and at each step
/// of its interval searches, and `checkpoint(p + 4)` before the independence
/// model's and at each step of its own (decision 799). A `Break` stops the run
/// there. What a stop means is the caller's.
pub fn run<B>(
    dataset: &Dataset,
    params: &SemParams,
    checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<SemOutcome, Halt<B>> {
    run_with_caps(dataset, params, [MAX_ITERATIONS; 3], checkpoint)
}

/// A solution: θ, F there, H⁻¹, and the steps taken.
#[derive(Debug, Clone)]
struct Solved {
    theta: Vec<f64>,
    f: f64,
    h_inverse: Vec<Vec<f64>>,
    iterations: u64,
}

/// What one attempt's loop is given at θ: F, g, H and, for Newton's, J.
struct Point {
    f: f64,
    g: Vec<f64>,
    h: Vec<Vec<f64>>,
    j: Option<Vec<Vec<f64>>>,
}

/// H⁻¹ g, as a vector.
fn solve(inverse_matrix: &[Vec<f64>], g: &[f64]) -> Vec<f64> {
    inverse_matrix.iter().map(|row| row.iter().zip(g).map(|(x, y)| x * y).sum()).collect()
}

/// Fisher scoring from `theta` (decision 745's step and line search). `Ok(Err(n))`
/// when the attempt ends without a solution after n steps: F undefined at the
/// start, H failing its Cholesky factor, no halving accepted, or the cap.
fn fisher<E>(
    mut theta: Vec<f64>,
    cap: u32,
    point: &dyn Fn(&[f64]) -> Option<Point>,
    f_at: &dyn Fn(&[f64]) -> Option<f64>,
    at: &mut impl FnMut() -> Result<(), E>,
) -> Result<Result<Solved, u64>, E> {
    let mut iterations = 0u64;
    loop {
        at()?;
        let Some(e) = point(&theta) else { return Ok(Err(iterations)) };
        let Ok(lh) = cholesky(&e.h) else { return Ok(Err(iterations)) };
        let h_inverse = spd_inverse(&lh);
        let step: Vec<f64> = solve(&h_inverse, &e.g).iter().map(|x| -x).collect();
        let decrement: f64 = -e.g.iter().zip(&step).map(|(x, y)| x * y).sum::<f64>();
        if decrement <= DECREMENT_STOP {
            return Ok(Ok(Solved { theta, f: e.f, h_inverse, iterations }));
        }
        if iterations >= u64::from(cap) {
            return Ok(Err(iterations));
        }
        let mut alpha = 1.0;
        let mut next = None;
        for _ in 0..=MAX_HALVINGS {
            let candidate: Vec<f64> = theta.iter().zip(&step).map(|(t, d)| t + alpha * d).collect();
            if let Some(f) = f_at(&candidate) {
                if decrement <= DECREMENT_F_TEST || f <= e.f {
                    next = Some(candidate);
                    break;
                }
            }
            alpha /= 2.0;
        }
        let Some(candidate) = next else { return Ok(Err(iterations)) };
        theta = candidate;
        iterations += 1;
    }
}

/// Newton's method on J, damped (decision 798), from `theta`. At each iteration
/// it stops where H passes its Cholesky factor and gᵀH⁻¹g is at most
/// [`DECREMENT_STOP`], and that stop is a solution only where J is positive
/// definite: a minimum, not a saddle. Otherwise the step is −(J + μ D)⁻¹ g, D
/// H's diagonal each raised to at least [`DIAGONAL_FLOOR`] of its largest; μ
/// grows (to [`DAMPING_START`] from 0, then tenfold) while J + μ D fails its
/// factor at [`DAMPED_TOLERANCE`] or F at the step is undefined, or larger
/// unless the step is relaxed. A step is relaxed (decisions 812 and 823) only
/// where H passes its factor with gᵀH⁻¹g at most [`DECREMENT_F_TEST`] and the
/// solve's own decrement, gᵀ(J + μD)⁻¹g, is at most it too: so every relaxed
/// step has δᵀ(J + μD)δ ≤ 1e-10, and δᵀJδ ≤ 1e-10 where J is positive
/// semidefinite. Reading H alone, as decision 812 wrote it, bounded nothing,
/// since the step solves J + μD, not H (the review of 365b638, finding 1).
/// After [`MAX_SOLVES`] solves for one step the attempt ends; after a step μ
/// falls (to 0 at or below [`DAMPING_START`], else a tenth). `at` is called at
/// each iteration and before each solve.
fn newton<E>(
    mut theta: Vec<f64>,
    cap: u32,
    point: &dyn Fn(&[f64]) -> Option<Point>,
    f_at: &dyn Fn(&[f64]) -> Option<f64>,
    at: &mut impl FnMut() -> Result<(), E>,
) -> Result<Result<Solved, u64>, E> {
    let mut iterations = 0u64;
    let mut mu = 0.0f64;
    loop {
        at()?;
        let Some(e) = point(&theta) else { return Ok(Err(iterations)) };
        let j = e.j.as_ref().expect("Newton's attempt evaluates J");
        // Decision 812: where H passes its factor and the decrement is at most
        // DECREMENT_F_TEST, F's rounding exceeds what a step can lower it by, so
        // a step whose own solve's decrement is at most it too (decision 823)
        // is taken whenever F at it is defined (the review of 9C-4, M1).
        let mut relaxed = false;
        if let Ok(lh) = cholesky(&e.h) {
            let h_inverse = spd_inverse(&lh);
            let decrement: f64 = solve(&h_inverse, &e.g).iter().zip(&e.g).map(|(x, y)| x * y).sum();
            if decrement <= DECREMENT_STOP {
                return Ok(match cholesky_within(j, 0.0) {
                    Ok(_) => Ok(Solved { theta, f: e.f, h_inverse, iterations }),
                    Err(_) => Err(iterations),
                });
            }
            relaxed = decrement <= DECREMENT_F_TEST;
        }
        if iterations >= u64::from(cap) {
            return Ok(Err(iterations));
        }
        let q = theta.len();
        let largest = (0..q).map(|a| e.h[a][a]).fold(0.0f64, f64::max);
        let d: Vec<f64> = (0..q).map(|a| e.h[a][a].max(DIAGONAL_FLOOR * largest)).collect();
        let mut accepted = None;
        for _ in 0..MAX_SOLVES {
            at()?;
            let damped: Vec<Vec<f64>> = (0..q).map(|a| (0..q).map(|b| if a == b { j[a][b] + mu * d[a] } else { j[a][b] }).collect()).collect();
            if let Ok(l) = cholesky_within(&damped, DAMPED_TOLERANCE) {
                let step = solve(&spd_inverse(&l), &e.g);
                let candidate: Vec<f64> = theta.iter().zip(&step).map(|(t, s)| t - s).collect();
                // Decision 823: the relaxation is bounded by this solve's own
                // decrement, gᵀ(J + μD)⁻¹g = δᵀ(J + μD)δ, so a relaxed step is
                // small in the measure it was solved in, not only in H's.
                let solved_decrement: f64 = step.iter().zip(&e.g).map(|(s, g)| s * g).sum();
                if let Some(f) = f_at(&candidate) {
                    if f <= e.f || (relaxed && solved_decrement <= DECREMENT_F_TEST) {
                        accepted = Some(candidate);
                        break;
                    }
                }
            }
            mu = if mu == 0.0 { DAMPING_START } else { 10.0 * mu };
        }
        let Some(candidate) = accepted else { return Ok(Err(iterations)) };
        theta = candidate;
        mu = if mu <= DAMPING_START { 0.0 } else { mu / 10.0 };
        iterations += 1;
    }
}

/// The starts (decision 798): attempt 1's, or with `triad` the triad starts,
/// each factor of three or more columns given CFA's triad φ₀ (decision 761) and
/// its loadings from it.
fn starts(model: &Model, s: &[Vec<f64>], triad: bool) -> Vec<f64> {
    let k = model.k;
    // r(v): a column itself, a factor its first column; as an index into S.
    let rep = |v: usize| -> usize {
        if v < k {
            model.factor_columns[v][0] - k
        } else {
            v - k
        }
    };
    let phi0: Vec<f64> = (0..k)
        .map(|f| {
            let cols = &model.factor_columns[f];
            let m = cols[0] - k;
            let half = s[m][m] / 2.0;
            if !triad || cols.len() < 3 {
                return half;
            }
            let (j, l) = (cols[1] - k, cols[2] - k);
            let t = s[m][j] * s[m][l] / s[j][l];
            if t.is_finite() && t > 0.0 && t < s[m][m] {
                t
            } else {
                half
            }
        })
        .collect();
    // Each path into t: the least-squares weights of r(t) on its sources'
    // representatives, from S; 0 where their block fails its factor.
    let mut path_start: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    let targets: BTreeSet<usize> = model.paths.iter().map(|(t, _)| *t).collect();
    for t in targets {
        let sources: Vec<usize> = model.paths.iter().filter(|(to, _)| *to == t).map(|(_, from)| *from).collect();
        let reps: Vec<usize> = sources.iter().map(|v| rep(*v)).collect();
        let block: Vec<Vec<f64>> = reps.iter().map(|a| reps.iter().map(|b| s[*a][*b]).collect()).collect();
        let towards: Vec<f64> = reps.iter().map(|a| s[*a][rep(t)]).collect();
        let weights = match cholesky(&block) {
            Ok(l) => solve(&spd_inverse(&l), &towards),
            Err(_) => vec![0.0; sources.len()],
        };
        for (from, w) in sources.iter().zip(weights) {
            path_start.insert((t, *from), w);
        }
    }
    let start_of = |e: Entry, kind: Kind| -> f64 {
        match (e, kind) {
            (Entry::A(t, f), Kind::Loading) => s[t - k][rep(f)] / phi0[f],
            (Entry::A(t, s_), _) => path_start[&(t, s_)],
            (Entry::Phi(v, w), _) if v == w => {
                if v < k {
                    phi0[v]
                } else if model.endogenous[v] {
                    s[v - k][v - k] / 2.0
                } else {
                    s[v - k][v - k]
                }
            }
            (Entry::Phi(a, b), _) => {
                if model.endogenous[a] || model.endogenous[b] {
                    0.0
                } else if a >= k && b >= k {
                    s[a - k][b - k]
                } else {
                    s[rep(a)][rep(b)] / 2.0
                }
            }
        }
    };
    model.params.iter().zip(&model.kinds).map(|(entries, kind)| start_of(entries[0], *kind)).collect()
}

/// Decision 798's check at the solution: the largest √((H⁻¹)_aa) over the
/// parameter's natural unit, its first entry's: √(s_r(t)r(t) / s_r(s)r(s)) for
/// A[t][s], √(s_r(a)r(a) s_r(b)r(b)) for Φ[a][b].
fn solution_measure(model: &Model, s: &[Vec<f64>], h_inverse: &[Vec<f64>]) -> f64 {
    let k = model.k;
    let rep = |v: usize| -> usize {
        if v < k {
            model.factor_columns[v][0] - k
        } else {
            v - k
        }
    };
    model
        .params
        .iter()
        .enumerate()
        .map(|(a, entries)| {
            let unit = match entries[0] {
                Entry::A(t, from) => (s[rep(t)][rep(t)] / s[rep(from)][rep(from)]).sqrt(),
                Entry::Phi(x, y) => (s[rep(x)][rep(x)] * s[rep(y)][rep(y)]).sqrt(),
            };
            h_inverse[a][a].sqrt() / unit
        })
        .fold(0.0f64, |m, x| if x.is_nan() { f64::INFINITY } else { m.max(x) })
}

/// Decision 798's check at the solution: whether every estimate's √((H⁻¹)_aa) is at
/// most [`SOLUTION_LIMIT`] of its natural unit, so the data determine it.
fn check_at_solution(model: &Model, s: &[Vec<f64>], h_inverse: &[Vec<f64>]) -> bool {
    solution_measure(model, s, h_inverse) <= SOLUTION_LIMIT
}

/// [`run`] with each attempt's cap as an argument.
fn run_with_caps<B>(
    dataset: &Dataset,
    params: &SemParams,
    caps: [u32; 3],
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<SemOutcome, Halt<B>> {
    let checked = check(dataset, params);
    if let Some(e) = checked.errors.into_iter().next() {
        return Err(Halt::Refused(e));
    }
    let (cells, mut model) = checked.ready.expect("no error means every column is good");
    let mut at = |point: usize| match checkpoint(point) {
        ControlFlow::Break(reason) => Err(Halt::Stopped { reason }),
        ControlFlow::Continue(()) => Ok(()),
    };
    at(0)?;
    let rows = Rows::of(&cells, &params.missing_codes);
    let p = model.p;
    let (s, log_det_s) = sample_covariance(&rows, &model.columns, &mut at)?;
    model.in_units_of(&s);
    let estimation_point = 1 + p;
    let solved = estimate(&model, &s, log_det_s, caps, &mut || at(estimation_point))?;
    let (solved, attempt) = match solved {
        Ok(found) => found,
        Err((count, limit)) => {
            return Err(Halt::Refused(
                Diagnostic::error(SemCode::SemNotConverged).int(ParamKey::Count, count as i64).int(ParamKey::Limit, i64::from(limit)),
            ))
        }
    };
    at(estimation_point)?;
    if !check_at_solution(&model, &s, &solved.h_inverse) {
        return Err(Halt::Refused(Diagnostic::error(SemCode::SolutionNotIdentified)));
    }
    at(p + 2)?;
    outcome(params, &model, &rows, &s, log_det_s, &solved, attempt, &mut at)
}

/// S with divisor N, from two-pass means, column by column in alias order
/// (decision 797), calling `at(1 + i)` before column i, and ln|S|; `overflow` or
/// `sample_covariance_singular` naming the column.
fn sample_covariance<B>(
    rows: &Rows,
    columns: &[Alias],
    at: &mut impl FnMut(usize) -> Result<(), Halt<B>>,
) -> Result<(Vec<Vec<f64>>, f64), Halt<B>> {
    let p = columns.len();
    let n = rows.complete();
    let nf = n as f64;
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
    // As decision 762's: a mean or an entry of S that is not finite is
    // overflow, naming the first column whose mean or variance is not finite,
    // or else the row's column of the first covariance that is not.
    let overflow = (0..p).find(|i| !means[*i].is_finite() || !s[*i][*i].is_finite()).or_else(|| (0..p).find(|i| (0..*i).any(|j| !s[*i][j].is_finite())));
    if let Some(i) = overflow {
        return Err(Halt::Refused(Diagnostic::error(SemCode::Overflow).with(ParamKey::Column, column_param(columns[i]))));
    }
    let ls = cholesky(&s).map_err(|j| {
        Halt::Refused(Diagnostic::error(SemCode::SampleCovarianceSingular).with(ParamKey::Column, column_param(columns[j - 1])))
    })?;
    let log_det_s = log_det(&ls);
    Ok((s, log_det_s))
}

/// Decision 798's three attempts in order, each with its cap: Fisher scoring
/// from the starts, Newton's method from the same starts, and Fisher scoring
/// from the triad starts, made only when they differ from the first. `Ok(Err)`
/// carries the most iterations an attempt made and the largest cap among the
/// attempts made.
#[allow(clippy::type_complexity)]
fn estimate<E>(
    model: &Model,
    s: &[Vec<f64>],
    log_det_s: f64,
    caps: [u32; 3],
    at: &mut impl FnMut() -> Result<(), E>,
) -> Result<Result<(Solved, u32), (u64, u32)>, E> {
    let fisher_point = |theta: &[f64]| evaluate(model, s, log_det_s, theta, false).map(|e| Point { f: e.f, g: e.g, h: e.h, j: None });
    let newton_point = |theta: &[f64]| evaluate(model, s, log_det_s, theta, true).map(|e| Point { f: e.f, g: e.g, h: e.h, j: e.j });
    let f_at = |theta: &[f64]| discrepancy(model, s, log_det_s, theta);
    let first = starts(model, s, false);
    let mut made: Vec<(u64, u32)> = Vec::new();
    match fisher(first.clone(), caps[0], &fisher_point, &f_at, at)? {
        Ok(found) => return Ok(Ok((found, 1))),
        Err(n) => made.push((n, caps[0])),
    }
    match newton(first.clone(), caps[1], &newton_point, &f_at, at)? {
        Ok(found) => return Ok(Ok((found, 2))),
        Err(n) => made.push((n, caps[1])),
    }
    let triad = starts(model, s, true);
    if triad != first {
        match fisher(triad, caps[2], &fisher_point, &f_at, at)? {
            Ok(found) => return Ok(Ok((found, 3))),
            Err(n) => made.push((n, caps[2])),
        }
    }
    let count = made.iter().map(|m| m.0).max().unwrap_or(0);
    let limit = made.iter().map(|m| m.1).max().unwrap_or(0);
    Ok(Err((count, limit)))
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
        Stat::NotComputable(SemCode::TooSmall)
    } else {
        Stat::NotComputable(SemCode::Overflow)
    }
}

/// An estimate with its standard error, C.R. and two-tailed P.
fn tested(estimate: f64, se: f64) -> (Stat, Stat, Stat, Stat) {
    let cr = estimate / se;
    (stat(estimate), stat(se), stat(cr), stat(normal_two_tailed(cr)))
}

/// `x · √a / √b` where both are above 0, else `negative_variance`: a
/// standardized weight or effect (decision 799).
fn standardized(x: f64, a: f64, b: f64) -> Stat {
    if a > 0.0 && b > 0.0 {
        stat(x * a.sqrt() / b.sqrt())
    } else {
        Stat::NotComputable(SemCode::NegativeVariance)
    }
}

/// For each variable, the variables reachable from it along the single-headed
/// arrows, loadings and paths (decision 799's directed path of arrows).
fn reach(model: &Model) -> Vec<Vec<bool>> {
    let m = model.m;
    let mut r = vec![vec![false; m]; m];
    for (t, s, _, _) in &model.weights {
        r[*s][*t] = true;
    }
    for via in 0..m {
        for from in 0..m {
            if r[from][via] {
                for to in 0..m {
                    if r[via][to] {
                        r[from][to] = true;
                    }
                }
            }
        }
    }
    r
}

/// Every statistic of decision 799 at the solution, calling `at(p + 3)` before
/// the default model's fit and `at(p + 4)` before the independence model's,
/// each again at its interval searches' steps (decision 796).
#[allow(clippy::too_many_arguments)]
fn outcome<B>(
    params: &SemParams,
    model: &Model,
    rows: &Rows,
    s: &[Vec<f64>],
    log_det_s: f64,
    solved: &Solved,
    attempt: u32,
    at: &mut impl FnMut(usize) -> Result<(), Halt<B>>,
) -> Result<SemOutcome, Halt<B>> {
    let (p, k, m) = (model.p, model.k, model.m);
    let n_rows = rows.complete();
    let big_n = n_rows as f64;
    let n = big_n - 1.0;
    let theta = &solved.theta;
    let imp = model.implied(theta).expect("the solution's I − A was inverted");
    let (a, phi, omega) = (&imp.a, &imp.phi, &imp.omega);
    let se: Vec<f64> = (0..model.q()).map(|x| (2.0 * solved.h_inverse[x][x] / n).sqrt()).collect();
    let fixed = Stat::NotComputable(SemCode::FixedParameter);
    let negative = Stat::NotComputable(SemCode::NegativeVariance);

    let weights = model
        .weights
        .iter()
        .map(|(t, from, kind, index)| {
            let (estimate, se_w, cr, pv) = match index {
                Some(x) => tested(a[*t][*from], se[*x]),
                None => (stat(1.0), fixed, fixed, fixed),
            };
            SemWeight {
                to: model.name(*t),
                from: model.name(*from),
                kind: *kind,
                estimate,
                se: se_w,
                cr,
                p: pv,
                standardized: standardized(a[*t][*from], omega[*from][*from], omega[*t][*t]),
            }
        })
        .collect();
    let covariances = model
        .covariances
        .iter()
        .map(|(x, y, index)| {
            let (estimate, se_c, cr, pv) = tested(phi[*x][*y], se[*index]);
            let correlation = if phi[*x][*x] > 0.0 && phi[*y][*y] > 0.0 { stat(phi[*x][*y] / (phi[*x][*x] * phi[*y][*y]).sqrt()) } else { negative };
            SemCovarianceEstimate { a: model.name(*x), b: model.name(*y), estimate, se: se_c, cr, p: pv, correlation }
        })
        .collect();
    let variances = (0..m)
        .map(|v| {
            let (estimate, se_v, cr, pv) = tested(phi[v][v], se[model.variances[v]]);
            SemVariance { variable: model.name(v), error: model.endogenous[v], estimate, se: se_v, cr, p: pv }
        })
        .collect();
    let smc = (0..m)
        .filter(|v| model.endogenous[*v])
        .map(|v| SemSmc {
            variable: model.name(v),
            value: if omega[v][v] > 0.0 { stat(1.0 - phi[v][v] / omega[v][v]) } else { negative },
        })
        .collect();
    // Effects among the structural variables: the factors and the columns no
    // factor holds, each ordered pair joined by a directed path (decision 799).
    let reachable = reach(model);
    let structural: Vec<usize> = (0..m).filter(|v| *v < k || !model.indicator[*v]).collect();
    let mut effects = Vec::new();
    for &to in &structural {
        for &from in &structural {
            if to == from || !reachable[from][to] {
                continue;
            }
            let total = imp.b[to][from];
            let direct = a[to][from];
            let indirect = total - direct;
            let (sf, st) = (omega[from][from], omega[to][to]);
            effects.push(SemEffect {
                to: model.name(to),
                from: model.name(from),
                total: stat(total),
                direct: stat(direct),
                indirect: stat(indirect),
                total_standardized: standardized(total, sf, st),
                direct_standardized: standardized(direct, sf, st),
                indirect_standardized: standardized(indirect, sf, st),
            });
        }
    }

    at(p + 3)?;
    let sigma_l = cholesky_within(&imp.sigma, 0.0).expect("the solution's Σ is positive definite");
    let sigma_inverse = spd_inverse(&sigma_l);
    let q = model.q() as u64;
    // Decision 812: never reached, since identification refuses such a spec in
    // check (a pivoted rank is at most the moments); mapped, not unwrapped.
    let d = model_df(p, q).map_err(|count| Halt::Refused(Diagnostic::error(SemCode::ModelNotIdentified).int(ParamKey::Count, count as i64)))?;
    let input = FitInput {
        s,
        log_det_s,
        sigma: &imp.sigma,
        sigma_inverse: &sigma_inverse,
        f: solved.f,
        q,
        d,
        big_n,
        n,
    };
    let rows_fit = fit_rows(&input, &mut |point| {
        at(match point {
            FitPoint::Default => p + 3,
            FitPoint::Independence => p + 4,
        })
    })?;
    let fit = SemFit { model: row_of(rows_fit.model), saturated: row_of(rows_fit.saturated), independence: row_of(rows_fit.independence) };

    let mut notes = Vec::new();
    if (0..m).any(|v| phi[v][v] < 0.0) {
        notes.push(SemCode::NegativeVariance);
    }
    let mut block: BTreeSet<usize> = (0..k).collect();
    for (x, y, _) in &model.covariances {
        block.insert(*x);
        block.insert(*y);
    }
    let block: Vec<usize> = block.into_iter().collect();
    let phi_block: Vec<Vec<f64>> = block.iter().map(|x| block.iter().map(|y| phi[*x][*y]).collect()).collect();
    if !block.is_empty() && cholesky_within(&phi_block, 0.0).is_err() {
        notes.push(SemCode::CovarianceNotPositiveDefinite);
    }
    if (0..m).any(|v| reachable[v][v]) {
        notes.push(SemCode::Nonrecursive);
    }
    if fit.model.df == 0 {
        notes.push(SemCode::DfZero);
    }

    let mut exclusions = Vec::new();
    for (j, alias) in model.columns.iter().enumerate() {
        for (code, count) in [(SemCode::MissingValue, rows.excluded[j][0]), (SemCode::MissingCode, rows.excluded[j][1])] {
            if count > 0 {
                exclusions.push(Exclusion {
                    code,
                    count,
                    params: BTreeMap::from([(ParamKey::Column, column_param(*alias))]),
                });
            }
        }
    }

    Ok(SemOutcome {
        columns: model.columns.clone(),
        rows_used: n_rows as u64,
        indicators: params.indicators.clone(),
        weights,
        covariances,
        variances,
        smc,
        effects,
        fit,
        iterations: solved.iterations,
        attempt,
        exclusions,
        notes,
        limitations: LIMITATIONS.to_vec(),
    })
}

/// A measure of fit as this module's [`Stat`] (decision 796).
fn of_fit(v: FitValue) -> Stat {
    match v {
        FitValue::Value(x) => Stat::Value(x),
        FitValue::NotComputable(code) => Stat::NotComputable(match code {
            FitCode::DfZero => SemCode::DfZero,
            FitCode::CminZero => SemCode::CminZero,
            FitCode::IndependenceCminZero => SemCode::IndependenceCminZero,
            FitCode::TooFewRowsForBcc => SemCode::TooFewRowsForBcc,
            FitCode::TooSmall => SemCode::TooSmall,
            FitCode::Overflow => SemCode::Overflow,
            FitCode::DistributionOutOfRange => SemCode::DistributionOutOfRange,
        }),
    }
}

/// One row of `stats_fit` as [`SemModelFit`], field for field.
fn row_of(r: FitRow) -> SemModelFit {
    SemModelFit {
        npar: r.npar,
        cmin: of_fit(r.cmin),
        df: r.df,
        p: of_fit(r.p),
        cmin_df: of_fit(r.cmin_df),
        rmr: of_fit(r.rmr),
        gfi: of_fit(r.gfi),
        agfi: of_fit(r.agfi),
        pgfi: of_fit(r.pgfi),
        nfi: of_fit(r.nfi),
        rfi: of_fit(r.rfi),
        ifi: of_fit(r.ifi),
        tli: of_fit(r.tli),
        cfi: of_fit(r.cfi),
        pratio: of_fit(r.pratio),
        pnfi: of_fit(r.pnfi),
        pcfi: of_fit(r.pcfi),
        ncp: of_fit(r.ncp),
        ncp_lo90: of_fit(r.ncp_lo90),
        ncp_hi90: of_fit(r.ncp_hi90),
        fmin: of_fit(r.fmin),
        f0: of_fit(r.f0),
        f0_lo90: of_fit(r.f0_lo90),
        f0_hi90: of_fit(r.f0_hi90),
        rmsea: of_fit(r.rmsea),
        rmsea_lo90: of_fit(r.rmsea_lo90),
        rmsea_hi90: of_fit(r.rmsea_hi90),
        pclose: of_fit(r.pclose),
        aic: of_fit(r.aic),
        bcc: of_fit(r.bcc),
        bic: of_fit(r.bic),
        caic: of_fit(r.caic),
        ecvi: of_fit(r.ecvi),
        ecvi_lo90: of_fit(r.ecvi_lo90),
        ecvi_hi90: of_fit(r.ecvi_hi90),
        mecvi: of_fit(r.mecvi),
        hoelter05: of_fit(r.hoelter05),
        hoelter01: of_fit(r.hoelter01),
    }
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows used
/// are the complete rows; with the exclusions they add up to the data rows. The
/// output hash is SHA-256 of [`SemOutcome::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<SemParams>,
    dataset: &Dataset,
    result: &SemOutcome,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<SemCode> {
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
    use crate::stats_l1::dataset::{build_dataset, DataFingerprint, RawTable, TableRef};

    const WHEATON: &str = include_str!("../../tests/fixtures/stats_cfa/data/wheaton.csv");
    const EXAMPLE_8: &str = include_str!("../../tests/fixtures/stats_cfa/data/example8.csv");

    /// A CSV fixture as a dataset, its '#' provenance lines skipped.
    fn table(csv: &str) -> Dataset {
        let mut lines = csv.lines().filter(|l| !l.starts_with('#'));
        let headers: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
        let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
        build_dataset(RawTable::from_rows(headers, rows), DataFingerprint::of_bytes(csv.as_bytes(), "sem unit tests", TableRef::FirstTable)).unwrap()
    }

    fn params(json: &str) -> SemParams {
        serde_json::from_str(json).unwrap()
    }

    /// The model, S, ln|S|, N and the solution, as the run reaches them.
    fn solved(dataset: &Dataset, params: &SemParams) -> (Model, Vec<Vec<f64>>, f64, usize, Solved, u32) {
        let checked = check(dataset, params);
        assert!(checked.errors.is_empty(), "{:?}", checked.errors);
        let (cells, mut model) = checked.ready.unwrap();
        let rows = Rows::of(&cells, &params.missing_codes);
        let (p, n) = (model.p, rows.complete());
        let means: Vec<f64> = rows.values.iter().map(|x| x.iter().sum::<f64>() / n as f64).collect();
        let mut s = vec![vec![0.0; p]; p];
        for i in 0..p {
            for j in 0..=i {
                let mut v = 0.0;
                for r in 0..n {
                    v += (rows.values[i][r] - means[i]) * (rows.values[j][r] - means[j]);
                }
                s[i][j] = v / n as f64;
                s[j][i] = s[i][j];
            }
        }
        let log_det_s = log_det(&cholesky(&s).unwrap());
        model.in_units_of(&s);
        let (found, attempt) = estimate(&model, &s, log_det_s, [MAX_ITERATIONS; 3], &mut || Ok::<(), Infallible>(())).unwrap().unwrap();
        (model, s, log_det_s, n, found, attempt)
    }

    /// Decision 798: J, the observed information, against central differences
    /// of the analytic gradient, on a model with a loop, a pair of loadings
    /// held equal and a correlated error, at a point away from the solution.
    /// The gate is ten times the largest relative difference measured (15 Nov
    /// 2026, macOS aarch64): the differences' own error, at a step of 1e-5 of
    /// each parameter's size.
    #[test]
    fn the_observed_information_agrees_with_differences_of_the_gradient() {
        let spec = params(
            r#"{"indicators":[["C1","C2","C3"],["C4","C5","C6"]],
                "paths":[{"to":{"factor":1},"from":{"factor":2}},{"to":{"factor":1},"from":{"column":"C7"}},
                         {"to":{"factor":2},"from":{"factor":1}},{"to":{"factor":2},"from":{"column":"C8"}}],
                "covariances":[{"a":{"column":"C3"},"b":{"column":"C6"}},{"a":{"column":"C7"},"b":{"column":"C8"}}],
                "equal":[[{"loading":{"factor":1,"column":"C2"}},{"loading":{"factor":2,"column":"C5"}}]],
                "missingCodes":[]}"#,
        );
        let model = Model::of(&spec);
        assert_eq!((model.p, model.q()), (8, 19), "the premise: 3 free loadings (4, two held equal), 4 paths, 2 covariances, 10 variances, q 19");
        // S: the implied Σ at one point, its diagonal raised, so the point
        // evaluated below is not a solution and R is not 0.
        let mut g = SplitMix(7);
        let at_s: Vec<f64> = model.kinds.iter().map(|k| match k { Kind::Loading => 0.6 + g.unit(), Kind::Path => 0.4 * (g.unit() - 0.5), Kind::Covariance => 0.1, Kind::Variance => 1.0 + g.unit() }).collect();
        let mut s = model.implied(&at_s).unwrap().sigma;
        for (i, row) in s.iter_mut().enumerate() {
            row[i] += 0.3;
        }
        let log_det_s = log_det(&cholesky(&s).unwrap());
        let theta: Vec<f64> = at_s.iter().map(|x| x * 1.05 + 0.01).collect();
        let e = evaluate(&model, &s, log_det_s, &theta, true).unwrap();
        let j = e.j.unwrap();
        let q = model.q();
        let mut worst = 0.0f64;
        let scale = j.iter().flatten().fold(0.0f64, |m, x| m.max(x.abs()));
        for b in 0..q {
            let h = 1e-5 * theta[b].abs().max(1.0);
            let mut up = theta.clone();
            up[b] += h;
            let mut down = theta.clone();
            down[b] -= h;
            let gu = evaluate(&model, &s, log_det_s, &up, false).unwrap().g;
            let gd = evaluate(&model, &s, log_det_s, &down, false).unwrap().g;
            for a in 0..q {
                let numeric = (gu[a] - gd[a]) / (2.0 * h);
                worst = worst.max((numeric - j[a][b]).abs() / scale);
            }
        }
        assert!(worst <= GATE_J, "the largest difference over J's largest entry: {worst:e}");
        // J is symmetric by construction; the expected information is not J.
        assert!((0..q).any(|a| (0..q).any(|b| (j[a][b] - e.h[a][b]).abs() > 1e-6 * scale)), "J equals H away from the solution");
    }

    /// The gate of the test above (decision 798): ten times the largest
    /// relative difference measured, 5.308e-11 (logs/p2_measure2_1794724243.log),
    /// rounded up.
    const GATE_J: f64 = 5.4e-10;

    /// Decision 798: Newton's attempt, driven on a function of its own. At the
    /// origin of x² − y², with H the identity, the decrement is 0, so it stops
    /// at once, and J = diag(2, −2) is not positive definite: a saddle, refused.
    /// On x² + y² from (1, 1), one step reaches the minimum, accepted.
    #[test]
    fn newtons_attempt_refuses_a_saddle_and_accepts_a_minimum() {
        let identity = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let saddle = |t: &[f64]| {
            Some(Point { f: t[0] * t[0] - t[1] * t[1], g: vec![2.0 * t[0], -2.0 * t[1]], h: identity.clone(), j: Some(vec![vec![2.0, 0.0], vec![0.0, -2.0]]) })
        };
        let f_saddle = |t: &[f64]| Some(t[0] * t[0] - t[1] * t[1]);
        let mut calls = 0;
        let got = newton(vec![0.0, 0.0], 10, &saddle, &f_saddle, &mut || {
            calls += 1;
            Ok::<(), Infallible>(())
        })
        .unwrap();
        assert!(matches!(got, Err(0)), "the saddle is refused at once");
        assert_eq!(calls, 1, "one check point, at the one iteration");
        let bowl = |t: &[f64]| Some(Point { f: t[0] * t[0] + t[1] * t[1], g: vec![2.0 * t[0], 2.0 * t[1]], h: identity.clone(), j: Some(vec![vec![2.0, 0.0], vec![0.0, 2.0]]) });
        let f_bowl = |t: &[f64]| Some(t[0] * t[0] + t[1] * t[1]);
        let found = newton(vec![1.0, 1.0], 10, &bowl, &f_bowl, &mut || Ok::<(), Infallible>(())).unwrap().unwrap();
        // J⁻¹ through its Cholesky factor gives 0.5000000000000001, so the step lands within
        // rounding of 0, where the decrement is far below 1e-20.
        assert_eq!(found.iterations, 1);
        assert!(found.theta.iter().all(|x| x.abs() <= 4.0 * f64::EPSILON), "{:?}", found.theta);
    }

    /// Decision 812 (decision 808's third answer): a spec with more parameters
    /// than moments is refused by check's identification, before any fit, so
    /// model_df's Err is a guard the run never reaches. On two columns a path
    /// and a covariance between them, with both variances, are 4 parameters
    /// over 3 moments; and a 6-column spec with every pair of columns
    /// covarying and a path between two of them, 22 over 21.
    #[test]
    fn identification_refuses_a_spec_with_more_parameters_than_moments_first() {
        let wheaton = table(WHEATON);
        let two = params(r#"{"indicators":[],"paths":[{"to":{"column":"C2"},"from":{"column":"C1"}}],"covariances":[{"a":{"column":"C1"},"b":{"column":"C2"}}],"equal":[],"missingCodes":[]}"#);
        let mut covs = Vec::new();
        for a in 1..=6 {
            for b in a + 1..=6 {
                covs.push(format!(r#"{{"a":{{"column":"C{a}"}},"b":{{"column":"C{b}"}}}}"#));
            }
        }
        let six = params(&format!(r#"{{"indicators":[],"paths":[{{"to":{{"column":"C2"}},"from":{{"column":"C1"}}}}],"covariances":[{}],"equal":[],"missingCodes":[]}}"#, covs.join(",")));
        for (spec, p, q) in [(two, 2usize, 4usize), (six, 6, 22)] {
            let model = Model::of(&spec);
            assert_eq!((model.p, model.q()), (p, q), "the premise");
            assert!(crate::stats_fit::model_df(p, q as u64).is_err(), "the premise: q above the moments");
            let checked = check(&wheaton, &spec);
            assert_eq!(checked.errors.len(), 1);
            assert_eq!(checked.errors[0].code, SemCode::ModelNotIdentified);
            assert!(checked.ready.is_none());
        }
    }

    /// Decision 812 (the review of 9C-4, M1): near a minimum, where a step
    /// would lower F by less than F's rounding, Newton's attempt takes Fisher
    /// scoring's relaxation of the F test. F here is 1 + x² + y² plus 4 ulps
    /// of 1 at every point but the start, so F is flat to rounding and every
    /// step leaves it a few ulps above F at the start: the strict test f <= e.f
    /// rejects each solve until μ is so large that the step no longer moves θ,
    /// and the attempt stands at its start until its cap (measured at a8e53eba:
    /// refused after 10 iterations, the cap here, and 52 check points). With the
    /// relaxation, H = J = 2I passes its Cholesky factor and gᵀH⁻¹g = 4e-16
    /// is at most DECREMENT_F_TEST, so the first step is taken and lands on
    /// the minimum, where the decrement is 0 and J is positive definite.
    #[test]
    fn newtons_attempt_accepts_a_step_flat_to_rounding_near_a_minimum() {
        const START: [f64; 2] = [1e-8, 1e-8];
        let rounding = |t: &[f64]| if t == START { 0.0 } else { 4.0 * f64::EPSILON };
        let f = |t: &[f64]| 1.0 + t[0] * t[0] + t[1] * t[1] + rounding(t);
        let two = vec![vec![2.0, 0.0], vec![0.0, 2.0]];
        let point = |t: &[f64]| Some(Point { f: f(t), g: vec![2.0 * t[0], 2.0 * t[1]], h: two.clone(), j: Some(two.clone()) });
        let f_at = |t: &[f64]| Some(f(t));
        // The premise: every step from the start, however damped, lands above F there.
        assert!(f(&[0.0, 0.0]) > f(&START) && f(&[0.5e-8, 0.5e-8]) > f(&START));
        let mut calls = 0;
        let found = newton(START.to_vec(), 10, &point, &f_at, &mut || {
            calls += 1;
            Ok::<(), Infallible>(())
        })
        .unwrap();
        let found = found.unwrap_or_else(|n| panic!("refused after {n} iterations and {calls} check points"));
        assert_eq!(found.iterations, 1);
        assert!(found.theta.iter().all(|x| x.abs() <= 4.0 * f64::EPSILON), "{:?}", found.theta);
    }

    /// Decision 823 (N1; the review of 365b638, finding 1): the relaxation is
    /// bounded by the solve's own decrement. f(t) = 0.5e-6 t² + 1e3 max(0, 5 − t)⁴
    /// with g = 1e-6 t, H = 1 and J f's second derivative. At t = 10, gᵀH⁻¹g is
    /// at most DECREMENT_F_TEST, but J = 1e-6, so the undamped solve's step is
    /// t itself (to 0, where F is 6.25e5) and its decrement gᵀJ⁻¹g is 1e-4: a
    /// relaxation read from H alone accepts it. Bounded, that solve is refused
    /// for its rise, μ grows to DAMPING_START and the step stops at 9.90099. From
    /// t = 20 gᵀH⁻¹g is 4e-10, so no relaxation applies either way (19.80198).
    /// The cap is 1, so `point`'s second call is at the one step taken.
    #[test]
    fn newtons_relaxed_step_is_bounded_by_its_own_solves_decrement() {
        use std::cell::RefCell;
        let f = |t: f64| 0.5e-6 * t * t + 1e3 * (5.0 - t).max(0.0).powi(4);
        let second = |t: f64| 1e-6 + 12e3 * (5.0 - t).max(0.0).powi(2);
        // Each start is checked and its faults kept, so a fault that breaks both shows both.
        let mut faults: Vec<String> = Vec::new();
        for (start, expected) in [(10.0f64, 9.90099f64), (20.0, 19.80198)] {
            let seen: RefCell<Vec<f64>> = RefCell::new(Vec::new());
            let point = |t: &[f64]| {
                seen.borrow_mut().push(t[0]);
                Some(Point { f: f(t[0]), g: vec![1e-6 * t[0]], h: vec![vec![1.0]], j: Some(vec![vec![second(t[0])]]) })
            };
            let f_at = |t: &[f64]| Some(f(t[0]));
            // The premises: gᵀH⁻¹g at the start against DECREMENT_F_TEST, and J = 1e-6 there.
            let g0 = 1e-6 * start;
            assert!(g0 * g0 > DECREMENT_STOP);
            assert_eq!(g0 * g0 <= DECREMENT_F_TEST, start == 10.0, "gᵀH⁻¹g = {:e}", g0 * g0);
            assert_eq!(second(start), 1e-6);
            assert!(f(0.0) > f(start));
            let got = newton(vec![start], 1, &point, &f_at, &mut || Ok::<(), Infallible>(())).unwrap();
            let seen = seen.into_inner();
            if seen.len() != 2 {
                faults.push(format!("from {start}: {} points, {seen:?}", seen.len()));
                continue;
            }
            if seen[1] <= 5.0 {
                faults.push(format!("from {start} the step went to {}", seen[1]));
            } else if (seen[1] - expected).abs() > 1e-5 {
                faults.push(format!("from {start}: {} against {expected}", seen[1]));
            }
            if !matches!(got, Err(1)) {
                faults.push(format!("from {start}: the cap did not end the attempt after one step: {got:?}"));
            }
        }
        assert!(faults.is_empty(), "{faults:#?}");
    }

    /// Decision 823 (N1): where H fails its factor nothing is relaxed. H = 0,
    /// g = 1e-6, J = 1: D is 0 (H's diagonal), so every solve is the same,
    /// gᵀJ⁻¹g = 1e-12, and F at every point but the start is larger. The attempt
    /// ends after MAX_SOLVES rejected solves at iteration 0.
    #[test]
    fn newtons_attempt_relaxes_nothing_where_h_fails_its_factor() {
        let start = [3.0f64];
        let point = |_t: &[f64]| Some(Point { f: 1.0, g: vec![1e-6], h: vec![vec![0.0]], j: Some(vec![vec![1.0]]) });
        let f_at = |t: &[f64]| Some(if t == start { 1.0 } else { 2.0 });
        assert!(cholesky(&[vec![0.0]]).is_err(), "the premise: H fails its factor");
        let mut calls = 0u32;
        let got = newton(start.to_vec(), 10, &point, &f_at, &mut || {
            calls += 1;
            Ok::<(), Infallible>(())
        })
        .unwrap();
        assert!(matches!(got, Err(0)), "{got:?}");
        assert_eq!(calls, 1 + MAX_SOLVES, "one iteration and every solve");
    }

    /// Decision 823 (N1): both decrements must be small. H = 1e-6, g = 1e-6,
    /// J = 1: gᵀH⁻¹g = 1e-6 is above DECREMENT_F_TEST while the solve's own,
    /// gᵀ(J + μD)⁻¹g, is at most 1e-12; F is larger everywhere but the start.
    /// So no step that moves θ is taken. D here is 1e-6 (H's diagonal), so as μ
    /// grows the step shrinks until 3 − step rounds to 3 itself, where F equals
    /// F at the start and the strict test takes it: measured at 365b638, the
    /// attempt ends at its cap of 10 having never moved, `Err(10)`, not the
    /// `Err(0)` decision 823 expected. What the test pins is the decision's
    /// point: θ never leaves the start (with the gᵀH⁻¹g test removed, the first
    /// solve is taken, to 2.999999).
    #[test]
    fn newtons_attempt_relaxes_nothing_where_h_s_decrement_is_large() {
        use std::cell::RefCell;
        let start = [3.0f64];
        let seen: RefCell<Vec<f64>> = RefCell::new(Vec::new());
        let point = |t: &[f64]| {
            seen.borrow_mut().push(t[0]);
            Some(Point { f: 1.0, g: vec![1e-6], h: vec![vec![1e-6]], j: Some(vec![vec![1.0]]) })
        };
        let f_at = |t: &[f64]| Some(if t == start { 1.0 } else { 2.0 });
        let h_decrement = 1e-6 * 1e-6 / 1e-6;
        assert!(h_decrement > DECREMENT_F_TEST, "the premise: {h_decrement:e}");
        assert!(1e-6 * 1e-6 / 1.0 <= DECREMENT_F_TEST, "the premise: the solve's decrement at μ = 0");
        assert!(cholesky(&[vec![1e-6]]).is_ok(), "the premise: H passes its factor, so (c) is not (b)");
        let got = newton(start.to_vec(), 10, &point, &f_at, &mut || Ok::<(), Infallible>(())).unwrap();
        let seen = seen.into_inner();
        assert!(seen.iter().all(|t| *t == start[0]), "θ left the start: {seen:?}");
        assert!(matches!(got, Err(10)), "{got:?}");
    }

    /// AMOS's Example 6, Model B: its 120 "Critical Ratios for Differences
    /// between Parameters" (the User's Guide, pp. 112-113; decision 795), each
    /// (θ_i − θ_j) / √(V_ii + V_jj − 2 V_ij) with V = [(N − 1) H / 2]⁻¹, to the
    /// printed three decimals. No gate: a one-in-the-last-place difference
    /// fails. AMOS's par_1 to par_16 in θ's layout: the free loadings (powles67,
    /// powles71, SEI), the paths ((alien67, ses), (alien71, alien67), (alien71,
    /// ses)), the covariance, then the variances (alien67, alien71, ses,
    /// anomia67, powles67, anomia71, powles71, educatio, SEI).
    #[test]
    fn wheaton_model_bs_critical_ratios_for_differences_are_amoss_printed_output() {
        let data = table(WHEATON);
        let spec = params(
            r#"{"indicators":[["C1","C2"],["C3","C4"],["C5","C6"]],
                "paths":[{"to":{"factor":1},"from":{"factor":3}},{"to":{"factor":2},"from":{"factor":1}},{"to":{"factor":2},"from":{"factor":3}}],
                "covariances":[{"a":{"column":"C1"},"b":{"column":"C3"}}],"equal":[],"missingCodes":[]}"#,
        );
        let (model, _, _, n, found, attempt) = solved(&data, &spec);
        assert_eq!((model.q(), attempt), (16, 1));
        // θ's index of AMOS's par_1 … par_16.
        let par = [1usize, 0, 2, 4, 5, 3, 6, 9, 7, 8, 10, 11, 12, 13, 14, 15];
        let estimates = [".9708", "1.0266", "5.1639", ".6173", "-.2116", "-.5498", "1.8855", "6.8715", "4.7003", "3.8623", "5.0593", "2.2108", "4.8055", "2.6809", "2.7282", "266.5671"];
        // AMOS prints no leading zero: 0.877 as .877 and −0.186 as −.186, 10.579 as itself.
        let shown = |x: f64, digits: usize| {
            let s = format!("{x:.digits$}");
            match (s.strip_prefix("-0."), s.strip_prefix("0.")) {
                (Some(r), _) => format!("-.{r}"),
                (_, Some(r)) => format!(".{r}"),
                _ => s,
            }
        };
        for (i, want) in estimates.iter().enumerate() {
            assert_eq!(shown(found.theta[par[i]], 4), *want, "par_{}", i + 1);
        }
        let printed: [&[&str]; 15] = [
            &[".877"],
            &["9.883", "9.741"],
            &["-4.429", "-5.931", "-10.579"],
            &["-17.943", "-16.634", "-12.284", "-18.098"],
            &["-22.343", "-26.471", "-12.661", "-17.300", "-5.115"],
            &["3.903", "3.689", "-6.762", "5.056", "8.490", "10.124"],
            &["8.955", "8.866", "1.707", "9.576", "10.995", "11.797", "7.128"],
            &["8.364", "7.872", "-.714", "9.256", "11.311", "12.047", "5.388", "-2.996"],
            &["7.781", "8.040", "-2.362", "9.470", "11.683", "12.629", "4.668", "-4.112", "-1.624"],
            &["11.106", "11.705", "-.186", "11.969", "14.039", "15.431", "9.773", "-2.402", ".548", "2.308"],
            &["3.826", "3.336", "-5.599", "4.998", "7.698", "8.253", ".740", "-6.387", "-5.254", "-3.507", "-4.728"],
            &["10.425", "9.659", "-.621", "10.306", "12.713", "13.575", "8.318", "-2.695", ".169", "1.554", "-.507", "5.042"],
            &["4.697", "4.906", "-4.642", "6.353", "8.554", "9.602", "1.798", "-5.701", "-3.909", "-2.790", "-4.735", ".999", "-3.322"],
            &["3.393", "3.283", "-7.280", "4.019", "5.508", "5.975", "1.482", "-3.787", "-2.667", "-1.799", "-3.672", ".855", "-3.199", ".077"],
            &["14.615", "14.612", "14.192", "14.637", "14.687", "14.712", "14.563", "14.506", "14.439", "14.458", "14.387", "14.544", "14.400", "14.518", "14.293"],
        ];
        let nm1 = (n - 1) as f64;
        let v = |a: usize, b: usize| 2.0 * found.h_inverse[a][b] / nm1;
        let mut compared = 0;
        for (row, columns) in printed.iter().enumerate() {
            let i = row + 1;
            for (j, want) in columns.iter().enumerate() {
                let (a, b) = (par[i], par[j]);
                let cr = (found.theta[a] - found.theta[b]) / (v(a, a) + v(b, b) - 2.0 * v(a, b)).sqrt();
                assert_eq!(shown(cr, 3), *want, "par_{} against par_{}", i + 1, j + 1);
                compared += 1;
            }
        }
        assert_eq!(compared, 120);
    }

    /// Decision 798's check at the solution, at a constructed point: the CFA's
    /// accepted solution on cfa_cases.rs's Walsh columns (one factor of three
    /// columns over 32 rows, φ near 0 and the loadings' standard errors near
    /// 7.5e14; decision 772), taken from stats_cfa::compute and put in this
    /// module's layout, which is the CFA's for this model. Its measure is above
    /// 10⁸, so the check refuses it; at Example 8's solution it passes.
    #[test]
    fn the_check_at_the_solution_refuses_the_walsh_point_and_passes_example_8() {
        let cols = [
            [2.0, 0.0, 2.0, 0.0, 2.0, 0.0, 2.0, 0.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 0.0, -2.0, 0.0, -2.0, 0.0, -2.0, 0.0, -2.0],
            [-0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5, -0.5, -0.5, 1.5, 1.5, -1.5, -1.5, 0.5, 0.5],
            [0.5, 1.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, 0.5, 1.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5, -1.5, -0.5],
        ];
        let mut csv = String::from("x1,x2,x3\n");
        for r in 0..32 {
            csv.push_str(&format!("{:?},{:?},{:?}\n", cols[0][r], cols[1][r], cols[2][r]));
        }
        let data = table(&csv);
        let cfa = crate::stats_cfa::compute(
            &data,
            &crate::stats_cfa::CfaParams { indicators: vec![vec![Alias(crate::stats_l1::dataset::ColumnAlias(1)), Alias(crate::stats_l1::dataset::ColumnAlias(2)), Alias(crate::stats_l1::dataset::ColumnAlias(3))]], correlated_factors: true, missing_codes: vec![] },
        )
        .expect("the CFA accepts this point");
        let value = |s: crate::stats_cfa::Stat| match s {
            crate::stats_cfa::Stat::Value(x) => x,
            crate::stats_cfa::Stat::NotComputable(_) => panic!("not computable"),
        };
        let theta = vec![
            value(cfa.loadings[1].estimate),
            value(cfa.loadings[2].estimate),
            value(cfa.factor_variances[0].estimate),
            value(cfa.error_variances[0].estimate),
            value(cfa.error_variances[1].estimate),
            value(cfa.error_variances[2].estimate),
        ];
        let spec = params(r#"{"indicators":[["C1","C2","C3"]],"paths":[],"covariances":[],"equal":[],"missingCodes":[]}"#);
        let model = Model::of(&spec);
        assert_eq!(model.q(), 6);
        let s: Vec<Vec<f64>> = (0..3).map(|i| (0..3).map(|j| (0..32).map(|r| cols[i][r] * cols[j][r]).sum::<f64>() / 32.0 - (cols[i].iter().sum::<f64>() / 32.0) * (cols[j].iter().sum::<f64>() / 32.0)).collect()).collect();
        let e = evaluate(&model, &s, 0.0, &theta, false).expect("Σ is positive definite there");
        let l = cholesky_within(&e.h, 0.0).expect("H factors at 0");
        let h_inverse = spd_inverse(&l);
        let measure = solution_measure(&model, &s, &h_inverse);
        assert!(measure > SOLUTION_LIMIT, "the measure {measure:e} is not above 1e8");
        assert!(!check_at_solution(&model, &s, &h_inverse), "the check refuses the Walsh point");
        assert!((WALSH_MEASURE_LOW..=WALSH_MEASURE_HIGH).contains(&measure), "the measure {measure:e}");
        // Example 8's solution passes, through the run.
        let ex8 = params(r#"{"indicators":[["C1","C2","C3"],["C4","C5","C6"]],"paths":[],"covariances":[{"a":{"factor":1},"b":{"factor":2}}],"equal":[],"missingCodes":[]}"#);
        let (model8, s8, _, _, found, _) = solved(&table(EXAMPLE_8), &ex8);
        let at8 = solution_measure(&model8, &s8, &found.h_inverse);
        assert!(at8 <= SOLUTION_LIMIT, "Example 8 measures {at8:e}");
        assert!(check_at_solution(&model8, &s8, &found.h_inverse), "the check passes Example 8");
        assert!(compute(&table(EXAMPLE_8), &ex8).is_ok());
    }

    /// The measure at the Walsh point, as measured on 15 Nov 2026 (macOS
    /// aarch64), within a factor of 2 either way.
    /// Measured 3.2339012742943895e15 (logs/p2_first_1794724127.log).
    const WALSH_MEASURE_LOW: f64 = 1.6e15;
    const WALSH_MEASURE_HIGH: f64 = 6.5e15;

    /// Decision 798's two points for one spec, Wheaton's Model A (three loadings, three
    /// paths, nine variances in θ's order), against SplitMix64 computed independently in
    /// BigInt by node (~/gaply-corpus/os/phase9/9c4/find_first/ff_splitmix.mjs, its output
    /// logs/ff_splitmix.log): a loading 0.5 + u, a path u − 0.5, a variance 1 + u.
    #[test]
    fn the_identification_draws_are_splitmix64s_from_0x9c4_and_0x9c5() {
        let spec = params(
            r#"{"indicators":[["C1","C2"],["C3","C4"],["C5","C6"]],
                "paths":[{"to":{"factor":1},"from":{"factor":3}},{"to":{"factor":2},"from":{"factor":1}},{"to":{"factor":2},"from":{"factor":3}}],
                "covariances":[],"equal":[],"missingCodes":[]}"#,
        );
        let model = Model::of(&spec);
        assert_eq!(draw(&model, 0x9C4), [0.8759761625245296, 0.9045600089646795, 1.1483266749969545, -0.36649248674853785, -0.2782049571559523, -0.43589231398327455, 1.7572951545177147, 1.7850381171682455, 1.0528351383294938, 1.9830354773017032, 1.5750122120584817, 1.0427168974180474, 1.624371447710684, 1.9572891765291383, 1.2939093148398397]);
        assert_eq!(draw(&model, 0x9C5), [0.7542252988355233, 0.8038965936990492, 0.7934858310007349, -0.024990316316876537, -0.22856477536082132, 0.29976278748774343, 1.4018393880632332, 1.0832381787716518, 1.5205942378629824, 1.2950335817526932, 1.5959886854186758, 1.5951474959386152, 1.7555215125442543, 1.341434715489636, 1.9429284613476323]);
        assert_eq!(identified(&model), Ok(()));
    }

    /// Decision 799's negative_variance codes for a standardized weight and a
    /// correlation, and the notes negative_variance and
    /// covariance_not_positive_definite, at a solution built for them, as the CFA's unit
    /// test builds its: factor 1's variance −0.2, Σ positive definite through large error
    /// variances, S equal to Σ.
    #[test]
    fn a_factor_variance_below_0_leaves_its_standardized_values_not_computable_and_is_noted() {
        let spec = params(r#"{"indicators":[["C1","C2","C3"],["C4","C5","C6"]],"paths":[],"covariances":[{"a":{"factor":1},"b":{"factor":2}}],"equal":[],"missingCodes":[]}"#);
        let model = Model::of(&spec);
        // λ for columns 2, 3, 5, 6; the covariance 0.1; φ11 = −0.2, φ22 = 1; Θ = 4.
        let theta = vec![0.8, 0.9, 1.1, 0.7, 0.1, -0.2, 1.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0];
        assert_eq!(model.q(), theta.len());
        let sigma = model.implied(&theta).unwrap().sigma;
        let l = cholesky_within(&sigma, 0.0).expect("Σ is positive definite");
        let rows = Rows { values: vec![vec![0.0; 10]; 6], excluded: vec![[0, 0]; 6] };
        let q = model.q();
        let identity: Vec<Vec<f64>> = (0..q).map(|i| (0..q).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
        let solved = Solved { theta, f: 0.0, h_inverse: identity, iterations: 0 };
        let r = outcome(&spec, &model, &rows, &sigma, log_det(&l), &solved, 1, &mut |_| Ok::<(), Halt<Infallible>>(())).unwrap();
        let negative = Stat::NotComputable(SemCode::NegativeVariance);
        for w in &r.weights[..3] {
            assert_eq!(w.standardized, negative, "a loading of factor 1");
        }
        assert!(matches!(r.weights[3].standardized, Stat::Value(_)));
        assert_eq!(r.covariances[0].correlation, negative);
        assert_eq!(r.notes, vec![SemCode::NegativeVariance, SemCode::CovarianceNotPositiveDefinite]);
        // The SMCs are given wherever Ω_vv > 0, below 0 here for factor 1's columns.
        assert!(matches!(r.smc[0].value, Stat::Value(x) if x < 0.0));
        assert!(r.effects.is_empty(), "no paths, no effects");
    }
}
