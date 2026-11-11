//! **`gaply_core::stats_factor`: exploratory factor analysis, as SPSS's FACTOR
//! does it (Research OS, Phase 9B-1, decisions 700 and 701).** Three to 128
//! numeric items in the dataset's column order; Pearson's correlations over the
//! complete rows; the determinant, KMO and each item's sampling adequacy,
//! Bartlett's test of sphericity; every eigenvalue of the correlation matrix with
//! its share of the variance; principal components or principal axis factoring,
//! for a number of factors the user gives or for every eigenvalue above 1; and,
//! with two factors or more, the loadings rotated by varimax or promax.
//!
//! **Each algorithm from its published statement** (decision 700; ADR-008), not
//! from any tool's source: Kaiser's (1958) varimax with his row normalisation,
//! iterated through the polar factor of the criterion's gradient; Hendrickson and
//! White's (1964) promax at power 4; principal axis factoring as SPSS states it,
//! from the squared multiple correlations until the largest change in a
//! communality is below the spec's convergence. R 4.6.1's `varimax` and `promax`
//! and psych 2.6.9 are the oracles (`tests/factor_r_golden.rs`,
//! `tests/fixtures/stats_factor/`), run outside Gaply.
//!
//! **Signs.** Every eigenvector's sign is set by `stats_linalg`'s rule (decision
//! 699), so an unrotated factor's loadings sum to more than 0. After a rotation,
//! each factor whose loadings' sum is below 0 is reversed by the same rule, its
//! pattern, structure and transformation columns, and its row and column of the
//! factor correlations, with it. No factor is reordered.
//!
//! **Not computable, statistic by statistic.** `correlations_singular` when the
//! correlation matrix's Cholesky factor refuses (decision 699's rule): the
//! determinant, KMO, the sampling adequacies, Bartlett's test and the squared
//! multiple correlations then come from no meaningful inverse. `too_small` for a
//! determinant below the smallest normal double; `distribution_out_of_range`;
//! `overflow`.
//!
//! **Pure** (`tests/contracts_are_pure.rs`): no file, network, process, clock or
//! model. The run's check points are the caller's: before the arithmetic, before
//! each item's row, before each correlation, before the extraction and before
//! the rotation; every iteration of either calls the check point again at its
//! unchanged count, so a cancel or the run's ceiling stops a long one.
//!
//! **Privacy.** The result names items by alias only and holds no label, no cell
//! and no matrix of the items by the items. The values never leave [`Rows`],
//! which derives nothing.

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
use crate::stats_alpha::{correlation, Stat as AlphaStat};
use crate::stats_desc::{
    input_ref, preparation_holds, score_code_count, suspect_codes_unless_score, Alias,
};
use crate::stats_dist::chi_square_upper_wide;
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_linalg::{
    cholesky, eigen_symmetric, least_squares, log_det, polar_factor, sign_reversed, spd_inverse,
};
use crate::stats_ttest::{column, non_finite_cells};
use crate::stats_verify::{mean, sample_variance, RangeFault};

pub mod export;

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.factor";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// The fewest items a run takes.
pub const MIN_ITEMS: usize = 3;
/// The most items a run takes (decision 700).
pub const MAX_ITEMS: usize = 128;
/// The most extraction iterations a spec may ask for (SPSS's ITERATE).
pub const MAX_ITERATIONS: u32 = 1000;
/// The convergence a spec may ask for, at least (SPSS's ECONVERGE).
pub const MIN_CONVERGENCE: f64 = 1e-12;
/// The convergence a spec may ask for, at most.
pub const MAX_CONVERGENCE: f64 = 0.1;
/// Varimax's iterations, always (decision 700): the spec's iterations limit the
/// extraction only.
pub const ROTATION_MAX_ITERATIONS: u64 = 1000;
/// Varimax stops when the criterion grows by less than this fraction (R's
/// `varimax` default, Kaiser's rule).
pub const VARIMAX_EPS: f64 = 1e-5;
/// Two eigenvalues within this fraction of the larger are not distinct
/// (decision 700's note).
pub const DISTINCT_TOLERANCE: f64 = 1e-8;

closed! {
    /// Every refusal, outcome, exclusion, note and limitation of the factor
    /// analysis.
    pub enum FactorCode in "stats.factor." {
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
        /// Fewer than three items. Params `count`, `limit`.
        TooFewItems => "too_few_items",
        /// More items than a run takes. Params `count`, `limit`.
        TooManyItems => "too_many_items",
        /// The items are not in the dataset's column order. Param `column`.
        ItemsOutOfOrder => "items_out_of_order",
        /// An item is named twice. Param `column`.
        ItemRepeated => "item_repeated",
        /// No column has that alias. Param `column`.
        UnknownColumn => "unknown_column",
        /// A column holds NaN or an infinity. Params `column`, `count`.
        ColumnNonFinite => "column_non_finite",
        /// A column holds text. Params `column`, `count`.
        ColumnHoldsText => "column_holds_text",
        /// A ticked missing code equals a value in a chosen score. Params
        /// `column`, `count`.
        CodeEqualsScore => "code_equals_score",
        /// The number of factors is outside 1 to the items (principal
        /// components) or to one fewer (principal axis). Params `count`, `limit`.
        FactorsOutOfRange => "factors_out_of_range",
        /// The iterations are outside 1 to 1,000. Params `count`, `limit`.
        IterationsOutOfRange => "iterations_out_of_range",
        /// The convergence is outside 1e-12 to 0.1, or not a number.
        ConvergenceOutOfRange => "convergence_out_of_range",
        /// No row has a number that is not a code in every item.
        NoCompleteRows => "no_complete_rows",
        /// Fewer than three complete rows. Params `count`, `limit`.
        TooFewRows => "too_few_rows",
        /// An item has one value over the complete rows. Param `column`, the
        /// first such.
        ItemConstant => "item_constant",
        /// A correlation cannot be held in a double. Param `column`, the pair's
        /// first item.
        CorrelationOutOfRange => "correlation_out_of_range",
        /// Factors by Kaiser's rule, and no eigenvalue is above 1.
        NoEigenvalueOverOne => "no_eigenvalue_over_one",
        /// The correlations are singular, to decision 699's rule: an item is a
        /// combination of the items before it. As a refusal (principal axis),
        /// param `column`, that item; as a statistic's code, no param.
        CorrelationsSingular => "correlations_singular",
        /// An eigenvalue among the m largest is 0 or below, so its root cannot
        /// be taken.
        ExtractionEigenvalueNotPositive => "extraction_eigenvalue_not_positive",
        /// Principal axis factoring did not converge within the iterations.
        /// Param `limit`.
        ExtractionNotConverged => "extraction_not_converged",
        /// An item's extracted loadings are all exactly 0, so the rotation's
        /// row normalisation cannot divide by its length. Param `column`.
        RotationItemWithoutLoadings => "rotation_item_without_loadings",
        /// Varimax did not converge within 1,000 iterations, or the rotation's
        /// arithmetic could not be completed.
        RotationNotConverged => "rotation_not_converged",
        /// The eigenvalues could not be found.
        EigenNotConverged => "eigen_not_converged",
        /// A candidate missing code the rule found in an item and the user did
        /// not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Not computable: the value is below the smallest normal double, or is
        /// a 0/0 of quantities that vanished (KMO or an MSA with no correlation
        /// at all, as R gives NaN).
        TooSmall => "too_small",
        /// Not computable: the distribution is not measured at these degrees of
        /// freedom.
        DistributionOutOfRange => "distribution_out_of_range",
        /// Not computable: a value along the way too large for a double.
        Overflow => "overflow",
        /// Note: a communality is above 1 (a Heywood case), as SPSS warns.
        CommunalityAboveOne => "communality_above_one",
        /// Note: one factor, so the rotation asked for was not made.
        OneFactorNotRotated => "one_factor_not_rotated",
        /// Note: the m-th and the next eigenvalue are equal within 1e-8 of the
        /// larger, so the loadings are one choice of many.
        EigenvaluesNotDistinct => "eigenvalues_not_distinct",
        /// Excluded: the item's cell is empty or a missing-value token.
        MissingValue => "missing_value",
        /// Excluded: the item's cell is a value the user listed.
        MissingCode => "missing_code",
        /// Limitation: maximum likelihood, least squares, alpha and image
        /// extraction are not offered.
        NoMlOrOtherExtraction => "no_ml_or_other_extraction",
        /// Limitation: oblimin, quartimax and equamax are not offered.
        NoObliqueOtherThanPromax => "no_oblique_other_than_promax",
        /// Limitation: promax's power is 4.
        PromaxKappaFour => "promax_kappa_four",
        /// Limitation: no factor scores.
        NoFactorScores => "no_factor_scores",
        /// Limitation: the correlation, anti-image, reproduced and residual
        /// matrices are not shown.
        NoMatricesShown => "no_matrices_shown",
        /// Limitation: Pearson's correlations only.
        PearsonOnly => "pearson_only",
        /// Limitation: listwise deletion only.
        ListwiseOnly => "listwise_only",
        /// Limitation: no scree plot.
        NoScreePlot => "no_scree_plot",
        /// Limitation: each row counts once; no weights.
        NoWeights => "no_weights",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// How the factors are extracted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Extraction {
    PrincipalComponents,
    PrincipalAxis,
}

/// How the extracted loadings are rotated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rotation {
    None,
    Varimax,
    Promax,
}

/// The factor analysis's parameters (decision 700): the items in the dataset's
/// column order; the extraction; the number of factors, or `None` for every
/// eigenvalue of the correlation matrix above 1; the rotation; the extraction's
/// iterations and convergence (SPSS's ITERATE and ECONVERGE); and the missing
/// codes ascending, which apply to every item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FactorParams {
    pub items: Vec<Alias>,
    pub extraction: Extraction,
    pub factors: Option<u32>,
    pub rotation: Rotation,
    pub iterations: u32,
    pub convergence: f64,
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
    NotComputable(FactorCode),
}

/// One item's mean and standard deviation (n − 1) over the complete rows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemDescriptive {
    pub item: Alias,
    pub mean: Stat,
    pub sd: Stat,
}

/// The correlation matrix's determinant and its natural log.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Determinant {
    pub value: Stat,
    pub log: Stat,
}

/// One item's measure of sampling adequacy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemAdequacy {
    pub item: Alias,
    pub msa: Stat,
}

/// Kaiser-Meyer-Olkin's measure, overall and for each item.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingAdequacy {
    pub overall: Stat,
    pub items: Vec<ItemAdequacy>,
}

/// Bartlett's test of sphericity.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sphericity {
    pub chi_square: Stat,
    pub df: u64,
    pub p: Stat,
}

/// A variance and its share: an eigenvalue, or a factor's sum of squared
/// loadings, with its percent of the items' total variance and the cumulative
/// percent.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactorSums {
    pub total: Stat,
    pub percent: Stat,
    pub cumulative: Stat,
}

/// One item's communality before and after the extraction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Communality {
    pub item: Alias,
    pub initial: Stat,
    pub extraction: Stat,
}

/// The extraction: its method, the m factors, its iterations (0 for principal
/// components), each item's communalities, each factor's sum of squared
/// loadings, and the loadings, one row per item of m values.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedFactors {
    pub method: Extraction,
    pub factors: u64,
    pub iterations: u64,
    pub communalities: Vec<Communality>,
    pub sums: Vec<FactorSums>,
    pub loadings: Vec<Vec<Stat>>,
}

/// The rotation: its method, varimax's iterations, and the loadings (for
/// promax, the pattern), one row per item; for varimax the transformation and
/// each factor's sums of squared loadings, for promax the structure and the
/// factor correlations. A field the method does not give is `None`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatedFactors {
    pub method: Rotation,
    pub iterations: u64,
    pub loadings: Vec<Vec<Stat>>,
    pub transformation: Option<Vec<Vec<Stat>>>,
    pub sums: Option<Vec<FactorSums>>,
    pub structure: Option<Vec<Vec<Stat>>>,
    pub factor_correlations: Option<Vec<Vec<Stat>>>,
}

/// The factor analysis's result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactorOutcome {
    pub items: Vec<Alias>,
    pub rows_used: u64,
    pub descriptives: Vec<ItemDescriptive>,
    pub determinant: Determinant,
    pub kmo: SamplingAdequacy,
    pub bartlett: Sphericity,
    /// Every eigenvalue of the correlation matrix, descending.
    pub eigenvalues: Vec<FactorSums>,
    pub extraction: ExtractedFactors,
    /// `None` with no rotation, or with one factor.
    pub rotation: Option<RotatedFactors>,
    /// The rows left out, by item and reason; none with no row is listed.
    pub exclusions: Vec<Exclusion<FactorCode>>,
    pub notes: Vec<FactorCode>,
    pub limitations: Vec<FactorCode>,
}

impl FactorOutcome {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, closed words and aliases serialises")
    }
}

/// What every result says it does not give, in decision 700's order.
const LIMITATIONS: [FactorCode; 9] = [
    FactorCode::NoMlOrOtherExtraction,
    FactorCode::NoObliqueOtherThanPromax,
    FactorCode::PromaxKappaFour,
    FactorCode::NoFactorScores,
    FactorCode::NoMatricesShown,
    FactorCode::PearsonOnly,
    FactorCode::ListwiseOnly,
    FactorCode::NoScreePlot,
    FactorCode::NoWeights,
];

/* ------------------------------------------------------------------------ *
 *  The one reading of the rows
 * ------------------------------------------------------------------------ */

/// The complete rows' values, one vector per item, and each item's rows left
/// out. Never leaves this module, and derives nothing.
struct Rows {
    values: Vec<Vec<f64>>,
    /// For each item: rows left out as `missing_value`, as `missing_code`.
    excluded: Vec<[u64; 2]>,
}

impl Rows {
    /// Listwise: a row is used only when every item holds a number that is not
    /// a code; a row left out is counted once, under the first item without
    /// one.
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
pub fn spec(
    operation_id: OperationId,
    dataset: &Dataset,
    parameters: FactorParams,
) -> OperationSpec<FactorParams> {
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

fn column_param(alias: Alias) -> ParamValue<FactorCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

/// The most factors the spec's extraction allows over `p` items.
fn factor_limit(extraction: Extraction, p: usize) -> usize {
    match extraction {
        Extraction::PrincipalComponents => p,
        Extraction::PrincipalAxis => p.saturating_sub(1),
    }
}

struct Checked<'d> {
    errors: Vec<Diagnostic<FactorCode>>,
    ready: Option<Vec<&'d [Cell]>>,
}

/// Shared by `validate` and `run` (decision 700's order): the missing codes;
/// the items' count; their order and a repeated item; each item (unknown, a
/// non-finite value, text); a code equal to a score's value; the factors; the
/// iterations; the convergence. Then, only when there is no other error, the
/// first of: no complete row, too few rows, an item constant.
fn check<'d>(dataset: &'d Dataset, params: &FactorParams) -> Checked<'d> {
    let mut errors = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(Diagnostic::error(FactorCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64));
        }
    }
    for (i, w) in params.missing_codes.windows(2).enumerate() {
        if w[0] == w[1] {
            errors.push(Diagnostic::error(FactorCode::MissingCodeRepeated).int(ParamKey::Index, i as i64 + 1));
        } else if w[0] > w[1] {
            errors.push(Diagnostic::error(FactorCode::MissingCodesOutOfOrder).int(ParamKey::Index, i as i64 + 1));
        }
    }
    let items = &params.items;
    let p = items.len();
    if p < MIN_ITEMS {
        errors.push(
            Diagnostic::error(FactorCode::TooFewItems)
                .int(ParamKey::Count, p as i64)
                .int(ParamKey::Limit, MIN_ITEMS as i64),
        );
    } else if p > MAX_ITEMS {
        errors.push(
            Diagnostic::error(FactorCode::TooManyItems)
                .int(ParamKey::Count, p as i64)
                .int(ParamKey::Limit, MAX_ITEMS as i64),
        );
    }
    for w in items.windows(2) {
        if w[0] > w[1] {
            errors.push(Diagnostic::error(FactorCode::ItemsOutOfOrder).with(ParamKey::Column, column_param(w[1])));
        }
    }
    for w in items.windows(2) {
        if w[0] == w[1] {
            errors.push(Diagnostic::error(FactorCode::ItemRepeated).with(ParamKey::Column, column_param(w[1])));
        }
    }
    let mut cells = Vec::with_capacity(p);
    for alias in items {
        let Some((summary, column_cells)) = column(dataset, *alias) else {
            errors.push(Diagnostic::error(FactorCode::UnknownColumn).with(ParamKey::Column, column_param(*alias)));
            continue;
        };
        let non_finite = non_finite_cells(summary);
        let texts = column_cells.iter().filter(|c| matches!(c, Cell::Text(_))).count() as i64;
        if non_finite > 0 {
            errors.push(
                Diagnostic::error(FactorCode::ColumnNonFinite)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, non_finite as i64),
            );
        } else if texts > 0 {
            errors.push(
                Diagnostic::error(FactorCode::ColumnHoldsText)
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
                Diagnostic::error(FactorCode::CodeEqualsScore)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, collisions as i64),
            );
        }
    }
    if let Some(m) = params.factors {
        let limit = factor_limit(params.extraction, p);
        if m == 0 || m as usize > limit {
            errors.push(
                Diagnostic::error(FactorCode::FactorsOutOfRange)
                    .int(ParamKey::Count, i64::from(m))
                    .int(ParamKey::Limit, limit as i64),
            );
        }
    }
    if params.iterations == 0 || params.iterations > MAX_ITERATIONS {
        errors.push(
            Diagnostic::error(FactorCode::IterationsOutOfRange)
                .int(ParamKey::Count, i64::from(params.iterations))
                .int(ParamKey::Limit, i64::from(MAX_ITERATIONS)),
        );
    }
    if !(params.convergence >= MIN_CONVERGENCE && params.convergence <= MAX_CONVERGENCE) {
        errors.push(Diagnostic::error(FactorCode::ConvergenceOutOfRange));
    }
    if !errors.is_empty() {
        return Checked { errors, ready: None };
    }
    let rows = Rows::of(&cells, &params.missing_codes);
    let n = rows.complete();
    if n == 0 {
        errors.push(Diagnostic::error(FactorCode::NoCompleteRows));
    } else if n < 3 {
        errors.push(
            Diagnostic::error(FactorCode::TooFewRows)
                .int(ParamKey::Count, n as i64)
                .int(ParamKey::Limit, 3),
        );
    } else if let Some(j) = rows.values.iter().position(|x| constant(x)) {
        errors.push(Diagnostic::error(FactorCode::ItemConstant).with(ParamKey::Column, column_param(items[j])));
    }
    Checked { errors, ready: Some(cells) }
}

/// Checks the spec against the dataset. Unsupported when it names another
/// capability, version or workspace; invalid on a wrong input or a parameter
/// error; a warning for each candidate missing code in an item that the user
/// did not list, a score's skipped. Nothing needs input.
pub fn validate(dataset: &Dataset, spec: &OperationSpec<FactorParams>) -> ValidationReport<FactorCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID {
        unsupported.push(Diagnostic::error(FactorCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(FactorCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(FactorCode::InputMismatch));
            }
        }
        inputs => errors.push(Diagnostic::error(FactorCode::InputCount).int(ParamKey::Count, inputs.len() as i64)),
    }
    if !preparation_holds(dataset, spec.preparation.as_ref()) {
        errors.push(Diagnostic::error(FactorCode::PreparationMismatch));
    }
    let params = &spec.parameters;
    errors.extend(check(dataset, params).errors);
    let mut warnings = Vec::new();
    let mut warned = BTreeSet::new();
    for alias in &params.items {
        if !warned.insert(*alias) {
            continue;
        }
        if let Some((_, cells)) = column(dataset, *alias) {
            for s in suspect_codes_unless_score(dataset, *alias, cells) {
                if !params.missing_codes.contains(&f64::from(s.code)) {
                    warnings.push(
                        Diagnostic::warning(FactorCode::SuspectMissingCode)
                            .with(ParamKey::Column, column_param(*alias))
                            .int(ParamKey::Value, i64::from(s.code))
                            .int(ParamKey::Count, s.cells as i64),
                    );
                }
            }
        }
    }
    let rows = dataset.rows() as u64;
    let columns = params.items.len() as u64;
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
    Refused(Diagnostic<FactorCode>),
    /// The caller's check point stopped the run.
    Stopped { reason: B },
}

/// How many check points a run of `params` calls (decision 700): one before the
/// arithmetic, one before each item's row, one before each of the p(p − 1)/2
/// correlations, one before the extraction and one before the rotation. The
/// iterations call the extraction's and the rotation's again, at an unchanged
/// count. Only the spec is needed.
pub fn check_points(params: &FactorParams) -> usize {
    let p = params.items.len();
    1 + p + p * p.saturating_sub(1) / 2 + 2
}

/// [`run`] with a check point that never stops.
pub fn compute(dataset: &Dataset, params: &FactorParams) -> Result<FactorOutcome, Diagnostic<FactorCode>> {
    match run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The factor analysis. `checkpoint(0)` is called after the parameters are
/// checked and before the arithmetic, `checkpoint(1..=p)` before each item's
/// row, `checkpoint(p + 1..)` before each correlation in the order (1, 2), (1,
/// 3), …, (p − 1, p), then once before the extraction and once before the
/// rotation; every extraction and rotation iteration calls its check point
/// again with the same count. A `Break` stops the run there. What a stop means
/// is the caller's.
pub fn run<B>(
    dataset: &Dataset,
    params: &FactorParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<FactorOutcome, Halt<B>> {
    let checked = check(dataset, params);
    if let Some(e) = checked.errors.into_iter().next() {
        return Err(Halt::Refused(e));
    }
    let cells = checked.ready.expect("no error means every item is good");
    let mut at = |point: usize| match checkpoint(point) {
        ControlFlow::Break(reason) => Err(Halt::Stopped { reason }),
        ControlFlow::Continue(()) => Ok(()),
    };
    at(0)?;
    let rows = Rows::of(&cells, &params.missing_codes);
    let items = &params.items;
    let p = items.len();
    let n = rows.complete();

    // Each item's mean and SD.
    let mut descriptives = Vec::with_capacity(p);
    for (i, values) in rows.values.iter().enumerate() {
        at(1 + i)?;
        let m = mean(values);
        let sd = match sample_variance(values) {
            Ok(v) => stat(v.sqrt()),
            Err(e) => Stat::NotComputable(range_code(RangeFault::of(&e))),
        };
        descriptives.push(ItemDescriptive {
            item: items[i],
            mean: stat(m),
            sd,
        });
    }

    // The correlations, in alpha's pair order, each from stats_alpha's own.
    let mut r = vec![vec![0.0; p]; p];
    let mut point = 1 + p;
    for i in 0..p {
        r[i][i] = 1.0;
        for j in i + 1..p {
            at(point)?;
            point += 1;
            match correlation(&rows.values[i], &rows.values[j]) {
                AlphaStat::Value(v) => {
                    r[i][j] = v;
                    r[j][i] = v;
                }
                AlphaStat::NotComputable(_) => {
                    return Err(Halt::Refused(
                        Diagnostic::error(FactorCode::CorrelationOutOfRange).with(ParamKey::Column, column_param(items[i])),
                    ))
                }
            }
        }
    }
    let extraction_point = point;
    let rotation_point = point + 1;

    // The Cholesky factor's statistics.
    let factor = cholesky(&r);
    let inverse = factor.as_ref().ok().map(|l| spd_inverse(l));
    let singular = Stat::NotComputable(FactorCode::CorrelationsSingular);
    let determinant = match &factor {
        Ok(l) => {
            let log = log_det(l);
            // Π l_jj², the determinant, in order; it never rises, so a value
            // below the smallest normal double is too small (decision 700).
            let value = l.iter().enumerate().fold(1.0f64, |d, (j, row)| d * (row[j] * row[j]));
            Determinant {
                value: if value < f64::MIN_POSITIVE { Stat::NotComputable(FactorCode::TooSmall) } else { stat(value) },
                log: stat(log),
            }
        }
        Err(_) => Determinant { value: singular, log: singular },
    };
    let kmo = match &inverse {
        Some(inv) => sampling_adequacy(items, &r, inv),
        None => SamplingAdequacy {
            overall: singular,
            items: items.iter().map(|a| ItemAdequacy { item: *a, msa: singular }).collect(),
        },
    };
    let df = (p * (p - 1) / 2) as u64;
    let bartlett = match &factor {
        Ok(l) => {
            // Hadamard: ln|R| ≤ 0 for a correlation matrix. Every a_jj is 1, so
            // each l_jj is at most 1 and this log is never above 0: the clamp
            // cannot fire, and is kept as a guard (the review of 9B-1).
            let log = log_det(l).min(0.0);
            let multiplier = n as f64 - 1.0 - (2.0 * p as f64 + 5.0) / 6.0;
            let chi = if log == 0.0 { 0.0 } else { -(multiplier * log) };
            let p_value = if !chi.is_finite() {
                Stat::NotComputable(FactorCode::Overflow)
            } else {
                match chi_square_upper_wide(chi, df as f64) {
                    Some(v) => stat(v),
                    None => Stat::NotComputable(FactorCode::DistributionOutOfRange),
                }
            };
            Sphericity { chi_square: stat(chi), df, p: p_value }
        }
        Err(_) => Sphericity { chi_square: singular, df, p: singular },
    };

    // Every eigenvalue of R.
    let (values, vectors) = eigen_symmetric(&r).ok_or_else(|| refuse(FactorCode::EigenNotConverged))?;
    let eigenvalues = sums_of(&values, p);
    let m = match params.factors {
        Some(m) => m as usize,
        None => {
            let above = values.iter().filter(|v| **v > 1.0).count();
            if above == 0 {
                return Err(refuse(FactorCode::NoEigenvalueOverOne));
            }
            above
        }
    };

    // The extraction.
    at(extraction_point)?;
    let mut notes = BTreeSet::new();
    let (loadings, communalities, iterations, final_values) = match params.extraction {
        Extraction::PrincipalComponents => {
            let loadings = loadings_of(&values, &vectors, m)?;
            let extracted = row_sums_of_squares(&loadings);
            let communalities = items
                .iter()
                .zip(&extracted)
                .map(|(a, h)| Communality { item: *a, initial: Stat::Value(1.0), extraction: stat(*h) })
                .collect::<Vec<_>>();
            (loadings, communalities, 0u64, values.clone())
        }
        Extraction::PrincipalAxis => {
            let inv = match (&factor, &inverse) {
                (Ok(_), Some(inv)) => inv,
                _ => {
                    let j = factor.as_ref().err().copied().unwrap_or(1);
                    return Err(Halt::Refused(
                        Diagnostic::error(FactorCode::CorrelationsSingular).with(ParamKey::Column, column_param(items[j - 1])),
                    ));
                }
            };
            let initial: Vec<f64> = (0..p).map(|i| 1.0 - 1.0 / inv[i][i]).collect();
            let mut h = initial.clone();
            let mut done = None;
            let mut last = None;
            for it in 1..=params.iterations {
                at(extraction_point)?;
                let mut reduced = r.clone();
                for (i, row) in reduced.iter_mut().enumerate() {
                    row[i] = h[i];
                }
                let (vals, vecs) = eigen_symmetric(&reduced).ok_or_else(|| refuse(FactorCode::EigenNotConverged))?;
                let loadings = loadings_of(&vals, &vecs, m)?;
                let next = row_sums_of_squares(&loadings);
                if next.iter().any(|x| *x > 1.0) {
                    notes.insert(FactorCode::CommunalityAboveOne);
                }
                let change = next.iter().zip(&h).fold(0.0f64, |c, (a, b)| c.max((a - b).abs()));
                h = next;
                last = Some((loadings, vals));
                if change < params.convergence {
                    done = Some(it);
                    break;
                }
            }
            let Some(it) = done else {
                return Err(Halt::Refused(
                    Diagnostic::error(FactorCode::ExtractionNotConverged).int(ParamKey::Limit, i64::from(params.iterations)),
                ));
            };
            let (loadings, vals) = last.expect("one iteration at least");
            let communalities = items
                .iter()
                .zip(initial.iter().zip(&h))
                .map(|(a, (s, e))| Communality { item: *a, initial: stat(*s), extraction: stat(*e) })
                .collect::<Vec<_>>();
            (loadings, communalities, u64::from(it), vals)
        }
    };
    if m < p {
        let (a, b) = (final_values[m - 1], final_values[m]);
        if (a - b).abs() <= DISTINCT_TOLERANCE * a.abs().max(b.abs()) {
            notes.insert(FactorCode::EigenvaluesNotDistinct);
        }
    }
    let extraction = ExtractedFactors {
        method: params.extraction,
        factors: m as u64,
        iterations,
        communalities,
        sums: sums_of(&column_sums_of_squares(&loadings), p),
        loadings: stats_of(&loadings),
    };

    // The rotation, its check point called with or without one, so a run
    // reaches every check point check_points counts.
    at(rotation_point)?;
    let rotation = match params.rotation {
        Rotation::None => None,
        _ if m == 1 => {
            notes.insert(FactorCode::OneFactorNotRotated);
            None
        }
        method => {
            if let Some(i) = loadings.iter().position(|row| row.iter().all(|x| *x == 0.0)) {
                return Err(Halt::Refused(
                    Diagnostic::error(FactorCode::RotationItemWithoutLoadings).with(ParamKey::Column, column_param(items[i])),
                ));
            }
            let (rotated, t, its) = varimax(&loadings, || at(rotation_point))?;
            Some(match method {
                Rotation::Varimax => varimax_result(rotated, t, its, p),
                _ => promax_result(&rotated, &t, its)?,
            })
        }
    };

    let mut exclusions = Vec::new();
    for (alias, counts) in items.iter().zip(&rows.excluded) {
        for (code, count) in [(FactorCode::MissingValue, counts[0]), (FactorCode::MissingCode, counts[1])] {
            if count > 0 {
                exclusions.push(Exclusion {
                    code,
                    count,
                    params: BTreeMap::from([(ParamKey::Column, column_param(*alias))]),
                });
            }
        }
    }
    // The notes in decision 700's order.
    let notes = [FactorCode::CommunalityAboveOne, FactorCode::OneFactorNotRotated, FactorCode::EigenvaluesNotDistinct]
        .into_iter()
        .filter(|c| notes.contains(c))
        .collect();
    Ok(FactorOutcome {
        items: items.clone(),
        rows_used: n as u64,
        descriptives,
        determinant,
        kmo,
        bartlett,
        eigenvalues,
        extraction,
        rotation,
        exclusions,
        notes,
        limitations: LIMITATIONS.to_vec(),
    })
}

fn refuse<B>(code: FactorCode) -> Halt<B> {
    Halt::Refused(Diagnostic::error(code))
}

/// `v` as a value; an infinity is `overflow`; a NaN, a 0/0 of quantities that
/// vanished, is `too_small`.
fn stat(v: f64) -> Stat {
    if v.is_finite() {
        Stat::Value(v)
    } else if v.is_nan() {
        Stat::NotComputable(FactorCode::TooSmall)
    } else {
        Stat::NotComputable(FactorCode::Overflow)
    }
}

fn stats_of(m: &[Vec<f64>]) -> Vec<Vec<Stat>> {
    m.iter().map(|row| row.iter().map(|x| stat(*x)).collect()).collect()
}

/// A refusal from `stats_verify`, by the range fault it carries.
fn range_code(fault: Option<RangeFault>) -> FactorCode {
    match fault {
        Some(RangeFault::Underflow) => FactorCode::TooSmall,
        _ => FactorCode::Overflow,
    }
}

/// Each variance with its percent of the p items' total and the cumulative
/// percent.
fn sums_of(totals: &[f64], p: usize) -> Vec<FactorSums> {
    let mut cumulative = 0.0;
    totals
        .iter()
        .map(|t| {
            let percent = t / p as f64 * 100.0;
            cumulative += percent;
            FactorSums { total: stat(*t), percent: stat(percent), cumulative: stat(cumulative) }
        })
        .collect()
}

/// Loadings v_j √λ_j for the m largest eigenvalues, each above 0.
fn loadings_of<B>(values: &[f64], vectors: &[Vec<f64>], m: usize) -> Result<Vec<Vec<f64>>, Halt<B>> {
    let p = vectors.len();
    if values[..m].iter().any(|v| !(*v > 0.0)) {
        return Err(refuse(FactorCode::ExtractionEigenvalueNotPositive));
    }
    let roots: Vec<f64> = values[..m].iter().map(|v| v.sqrt()).collect();
    Ok((0..p).map(|i| (0..m).map(|j| vectors[j][i] * roots[j]).collect()).collect())
}

fn row_sums_of_squares(m: &[Vec<f64>]) -> Vec<f64> {
    m.iter().map(|row| row.iter().map(|x| x * x).sum()).collect()
}

fn column_sums_of_squares(m: &[Vec<f64>]) -> Vec<f64> {
    let k = m.first().map_or(0, Vec::len);
    (0..k).map(|j| m.iter().map(|row| row[j] * row[j]).sum()).collect()
}

/// KMO and each item's MSA (decision 700): q_ij = −(R⁻¹)_ij / √((R⁻¹)_ii
/// (R⁻¹)_jj), and Σ r² / (Σ r² + Σ q²) over i ≠ j, overall and over row i.
fn sampling_adequacy(items: &[Alias], r: &[Vec<f64>], inv: &[Vec<f64>]) -> SamplingAdequacy {
    let p = r.len();
    let (mut r2, mut q2) = (0.0, 0.0);
    let mut per_item = Vec::with_capacity(p);
    for i in 0..p {
        let (mut ri, mut qi) = (0.0, 0.0);
        for j in 0..p {
            if j == i {
                continue;
            }
            let q = -inv[i][j] / (inv[i][i] * inv[j][j]).sqrt();
            ri += r[i][j] * r[i][j];
            qi += q * q;
        }
        r2 += ri;
        q2 += qi;
        per_item.push(ItemAdequacy { item: items[i], msa: stat(ri / (ri + qi)) });
    }
    SamplingAdequacy { overall: stat(r2 / (r2 + q2)), items: per_item }
}

/// Kaiser's varimax with his row normalisation (decision 700): each row over its
/// length h_i; T from the identity; each iteration Z = X T, B = Xᵀ(Z³ − Z
/// diag(1ᵀZ²)/p), T the polar factor of B and d the sum of B's singular values,
/// until d < d_prev (1 + 1e-5), within 1,000 iterations; the loadings X T, each
/// row times h_i. `tick` is the rotation's check point, called every iteration.
#[allow(clippy::type_complexity)]
fn varimax<B>(
    loadings: &[Vec<f64>],
    mut tick: impl FnMut() -> Result<(), Halt<B>>,
) -> Result<(Vec<Vec<f64>>, Vec<Vec<f64>>, u64), Halt<B>> {
    let p = loadings.len();
    let m = loadings[0].len();
    let h: Vec<f64> = loadings.iter().map(|row| row.iter().map(|x| x * x).sum::<f64>().sqrt()).collect();
    let x: Vec<Vec<f64>> = loadings.iter().zip(&h).map(|(row, hi)| row.iter().map(|v| v / hi).collect()).collect();
    let mut t: Vec<Vec<f64>> = (0..m).map(|i| (0..m).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    let mut d = 0.0f64;
    for it in 1..=ROTATION_MAX_ITERATIONS {
        tick()?;
        let z = multiply(&x, &t);
        let columns: Vec<f64> = (0..m).map(|k| z.iter().map(|row| row[k] * row[k]).sum()).collect();
        let inner: Vec<Vec<f64>> = z
            .iter()
            .map(|row| (0..m).map(|k| row[k] * row[k] * row[k] - row[k] * columns[k] / p as f64).collect())
            .collect();
        // B = Xᵀ inner, m × m.
        let b: Vec<Vec<f64>> = (0..m).map(|a| (0..m).map(|k| (0..p).map(|i| x[i][a] * inner[i][k]).sum()).collect()).collect();
        let (next, sum) = polar_factor(&b).ok_or_else(|| refuse(FactorCode::RotationNotConverged))?;
        t = next;
        let previous = d;
        d = sum;
        if d < previous * (1.0 + VARIMAX_EPS) {
            let rotated = multiply(&x, &t).into_iter().zip(&h).map(|(row, hi)| row.into_iter().map(|v| v * hi).collect()).collect();
            return Ok((rotated, t, it));
        }
    }
    Err(refuse(FactorCode::RotationNotConverged))
}

fn multiply(a: &[Vec<f64>], b: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let inner = b.len();
    let cols = b.first().map_or(0, Vec::len);
    a.iter().map(|row| (0..cols).map(|j| (0..inner).map(|k| row[k] * b[k][j]).sum()).collect()).collect()
}

fn transpose(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let cols = a.first().map_or(0, Vec::len);
    (0..cols).map(|j| a.iter().map(|row| row[j]).collect()).collect()
}

/// The factors to reverse after a rotation: each whose loadings' sum is below 0,
/// by `stats_linalg`'s rule.
fn reversed_factors(loadings: &[Vec<f64>]) -> Vec<bool> {
    transpose(loadings).iter().map(|col| sign_reversed(col)).collect()
}

fn reverse_columns(m: &mut [Vec<f64>], flip: &[bool]) {
    for row in m.iter_mut() {
        for (x, f) in row.iter_mut().zip(flip) {
            if *f {
                *x = -*x;
            }
        }
    }
}

fn varimax_result(mut loadings: Vec<Vec<f64>>, mut t: Vec<Vec<f64>>, iterations: u64, p: usize) -> RotatedFactors {
    let flip = reversed_factors(&loadings);
    reverse_columns(&mut loadings, &flip);
    reverse_columns(&mut t, &flip);
    RotatedFactors {
        method: Rotation::Varimax,
        iterations,
        sums: Some(sums_of(&column_sums_of_squares(&loadings), p)),
        loadings: stats_of(&loadings),
        transformation: Some(stats_of(&t)),
        structure: None,
        factor_correlations: None,
    }
}

/// Hendrickson and White's promax at power 4 (decision 700), from varimax's
/// loadings A and transformation T: the target Q = A ∘ |A|³; U the least
/// squares of A U = Q, no intercept; U scaled by √diag((UᵀU)⁻¹); the pattern
/// A U, the whole transformation W = T U, the factor correlations Φ = (WᵀW)⁻¹
/// and the structure the pattern times Φ.
fn promax_result<B>(a: &[Vec<f64>], t: &[Vec<f64>], iterations: u64) -> Result<RotatedFactors, Halt<B>> {
    let failed = || refuse(FactorCode::RotationNotConverged);
    let q: Vec<Vec<f64>> = a.iter().map(|row| row.iter().map(|v| v * v.abs() * v.abs() * v.abs()).collect()).collect();
    let u = least_squares(a, &q).ok_or_else(failed)?;
    let utu = multiply(&transpose(&u), &u);
    let d = spd_inverse(&cholesky(&utu).map_err(|_| failed())?);
    let scale: Vec<f64> = (0..d.len()).map(|j| d[j][j].sqrt()).collect();
    let u: Vec<Vec<f64>> = u.iter().map(|row| row.iter().zip(&scale).map(|(v, s)| v * s).collect()).collect();
    let mut pattern = multiply(a, &u);
    let mut w = multiply(t, &u);
    let wtw = multiply(&transpose(&w), &w);
    let mut phi = spd_inverse(&cholesky(&wtw).map_err(|_| failed())?);
    let flip = reversed_factors(&pattern);
    reverse_columns(&mut pattern, &flip);
    reverse_columns(&mut w, &flip);
    reverse_columns(&mut phi, &flip);
    for (row, f) in phi.iter_mut().zip(&flip) {
        if *f {
            row.iter_mut().for_each(|x| *x = -*x);
        }
    }
    let structure = multiply(&pattern, &phi);
    Ok(RotatedFactors {
        method: Rotation::Promax,
        iterations,
        loadings: stats_of(&pattern),
        transformation: None,
        sums: None,
        structure: Some(stats_of(&structure)),
        factor_correlations: Some(stats_of(&phi)),
    })
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows used
/// are the complete rows; with the exclusions they add up to the data rows. The
/// output hash is SHA-256 of [`FactorOutcome::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<FactorParams>,
    dataset: &Dataset,
    result: &FactorOutcome,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<FactorCode> {
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

/// What no dataset reaches, or a bound decision 700 states, with crafted
/// numbers.
#[cfg(test)]
mod tests {
    use super::*;

    /// Decision 700: with n ≥ p + 1 rows, which a positive-definite R needs,
    /// Bartlett's multiplier n − 1 − (2p + 5)/6 is at least (4p − 5)/6 > 0.
    #[test]
    fn bartletts_multiplier_is_above_0_whenever_the_rows_are_more_than_the_items() {
        for p in MIN_ITEMS..=MAX_ITEMS {
            let n = p + 1;
            let multiplier = n as f64 - 1.0 - (2.0 * p as f64 + 5.0) / 6.0;
            assert!(multiplier >= (4.0 * p as f64 - 5.0) / 6.0 - 1e-12 && multiplier > 0.0, "p {p}: {multiplier}");
        }
    }

    #[test]
    fn a_nan_is_too_small_and_an_infinity_overflow() {
        assert_eq!(stat(f64::NAN), Stat::NotComputable(FactorCode::TooSmall));
        assert_eq!(stat(f64::INFINITY), Stat::NotComputable(FactorCode::Overflow));
        assert_eq!(stat(-0.5), Stat::Value(-0.5));
    }
}
