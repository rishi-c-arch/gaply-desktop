//! **`gaply_core::stats_factor`: exploratory factor analysis, as SPSS's FACTOR
//! does it (Research OS, Phase 9B-1, decisions 700 and 701).** Three to 128
//! numeric items in the dataset's column order; Pearson's correlations over the
//! complete rows; the determinant, KMO and each item's sampling adequacy,
//! Bartlett's test of sphericity; every eigenvalue of the correlation matrix with
//! its share of the variance; principal components or principal axis factoring,
//! for a number of factors the user gives or for every eigenvalue above 1; and,
//! with two factors or more, the loadings rotated by varimax or promax.
//!
//! **Each algorithm as SPSS computes it** (decisions 710, 715 to 717), from IBM's
//! FACTOR algorithms document (SPSS 14.0) and SPSS's own solutions, not from any
//! tool's source (ADR-008): varimax by Kaiser's pairwise rotations with his row
//! normalisation, in cycles until the criterion changes by at most 1e-4, within
//! the spec's iterations (SPSS's ITERATE limits the rotation too); promax at
//! power 4 from a row-normalised target; principal axis factoring from the
//! squared multiple correlations, with the absolute value of any eigenvalue
//! below 0 among the m largest, until the largest change in a communality is
//! below the spec's convergence. The oracles, run outside Gaply: EFAtools 1.1.0's
//! SPSS route and a second route in base R (`tests/factor_r_golden.rs`), SPSS's
//! own solutions as EFAtools ships them (`spss_tier.rs`), and psych 2.6.9 for
//! KMO, Bartlett's test and principal axis.
//!
//! **Signs and order.** Every eigenvector's sign is set by `stats_linalg`'s rule
//! (decision 699), so an unrotated factor's loadings sum to more than 0. After
//! varimax, each factor whose loadings' sum is below 0 is reversed by the same
//! rule with its column of T, and the factors are sorted by their sums of
//! squares, largest first (IBM, p. 12, steps f and g). Promax starts from that
//! varimax and neither reflects nor sorts after its fit.
//!
//! **A rotation that cannot be given is a note** (decision 717): varimax not
//! meeting its criterion within the iterations (`rotation_not_converged`), or
//! promax giving no rotation (`rotation_singular`): a varimax column at most
//! 1e-7 times the longest (decision 728), or its least squares or an inverse
//! that cannot be computed. The result keeps everything else, its rotation
//! `None`.
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
//! the rotation; every extraction iteration and every varimax cycle calls its
//! check point again at its unchanged count, so a cancel or the run's ceiling
//! stops a long one.
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
    cholesky, eigen_symmetric, least_squares, log_det, multiply, sign_reversed, spd_inverse, transpose,
    LEAST_SQUARES_RANK_TOLERANCE,
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
/// The most iterations a spec may ask for (SPSS's ITERATE): principal axis's
/// iterations and varimax's cycles alike (decision 715).
pub const MAX_ITERATIONS: u32 = 1000;
/// The convergence a spec may ask for, at least (SPSS's ECONVERGE).
pub const MIN_CONVERGENCE: f64 = 1e-12;
/// The convergence a spec may ask for, at most.
pub const MAX_CONVERGENCE: f64 = 0.1;
/// Varimax stops at the first cycle whose criterion SV (not divided by p²)
/// changes by at most this (decision 715): RCONVERGE's default in IBM's FACTOR
/// syntax. Inferred, not documented as such: over EFAtools' 17 SPSS solutions
/// SPSS stopped at the first cycle where it held, in every one, while IBM's
/// algorithms document (14.0, p. 12) states 1e-5 on SV divided by p², which fits
/// 3 of the 17. The cycles are limited by the spec's iterations, as SPSS's
/// ITERATE limits the rotation too.
pub const VARIMAX_CRITERION: f64 = 1e-4;
/// A row whose Kaiser weight or promax target norm is below this is left
/// unscaled (decision 715).
pub const ROW_NORM_FLOOR: f64 = 1e-15;
/// A pair of factors is not rotated when |sin P| is at most this (IBM, p. 11).
pub const ROTATION_SINE_FLOOR: f64 = 1e-15;
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
        /// A correlation cannot be held in a double. Param `column`, the first
        /// item whose own variance cannot be given, else the pair's first item.
        CorrelationOutOfRange => "correlation_out_of_range",
        /// Factors by Kaiser's rule, and no eigenvalue is above 1.
        NoEigenvalueOverOne => "no_eigenvalue_over_one",
        /// The correlations are singular, to decision 699's rule: an item is a
        /// combination of the items before it. As a refusal (principal axis),
        /// param `column`, that item; as a statistic's code, no param.
        CorrelationsSingular => "correlations_singular",
        /// Principal components: an eigenvalue among the m largest is 0 or
        /// below, so its root cannot be taken.
        ExtractionEigenvalueNotPositive => "extraction_eigenvalue_not_positive",
        /// Principal axis factoring did not converge within the iterations.
        /// Param `limit`.
        ExtractionNotConverged => "extraction_not_converged",
        /// Note: varimax did not meet its criterion within the spec's
        /// iterations (SPSS's ITERATE limits the rotation too), so no rotation
        /// is given; everything else is (decision 717).
        RotationNotConverged => "rotation_not_converged",
        /// Note: promax gives no rotation when a varimax column is at most
        /// 1e-7 times the longest (decision 728), or when its least squares or
        /// an inverse cannot be computed; everything else is given (decision
        /// 717).
        RotationSingular => "rotation_singular",
        /// Note: an iteration of principal axis factoring used the absolute
        /// value of an eigenvalue below 0 among the m largest, as SPSS does
        /// (decision 717).
        ExtractionEigenvalueNegative => "extraction_eigenvalue_negative",
        /// The eigenvalues could not be found.
        EigenNotConverged => "eigen_not_converged",
        /// A candidate missing code the rule found in an item and the user did
        /// not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Not computable: the value is below the smallest normal double, or is
        /// a 0/0 of quantities that vanished.
        TooSmall => "too_small",
        /// Not computable: a KMO or an item's MSA is 0/0, since no item it
        /// counts is correlated with another at all (R gives NaN; decision 714).
        NoCorrelation => "no_correlation",
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
/// eigenvalue of the correlation matrix above 1; the rotation; the iterations
/// (SPSS's ITERATE, limiting principal axis's iterations and varimax's cycles
/// alike, decision 715) and the extraction's convergence (ECONVERGE); and the
/// missing codes ascending, which apply to every item.
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

/// The rotation: its method, varimax's cycles (promax's too, which starts from
/// it), and the loadings (for promax, the pattern), one row per item; for varimax
/// the transformation and each factor's sums of squared loadings, for promax the
/// structure and the factor correlations. A field the method does not give is
/// `None`.
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
    /// `None` with no rotation, with one factor, or when the rotation could not
    /// be given (a note says why).
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
/// extraction's iterations and varimax's cycles call theirs again, at an
/// unchanged count. Only the spec is needed.
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
/// rotation; every extraction iteration and every varimax cycle calls its
/// check point again with the same count. A `Break` stops the run there. What a
/// stop means is the caller's.
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
    // The first item whose own variance the arithmetic cannot give: the item a
    // correlation's refusal names (decision 714).
    let mut first_failing = None;
    for (i, values) in rows.values.iter().enumerate() {
        at(1 + i)?;
        let m = mean(values);
        let sd = match sample_variance(values) {
            Ok(v) => stat(v.sqrt()),
            Err(e) => {
                first_failing.get_or_insert(i);
                Stat::NotComputable(range_code(RangeFault::of(&e)))
            }
        };
        descriptives.push(ItemDescriptive {
            item: items[i],
            mean: stat(m),
            sd,
        });
    }

    // The correlations, in alpha's pair order, each from stats_alpha's own. A
    // pair that cannot be given names the first item, in the dataset's order,
    // whose own sum of squares or variance cannot be given (`pearson_r`'s checks
    // of one item are `sample_variance`'s); when every item's can, the pair's
    // first (decision 714).
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
                        Diagnostic::error(FactorCode::CorrelationOutOfRange)
                            .with(ParamKey::Column, column_param(items[first_failing.unwrap_or(i)])),
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
            let loadings = component_loadings(&values, &vectors, m)?;
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
                let (loadings, negative) = axis_loadings(&vals, &vecs, m);
                if negative {
                    notes.insert(FactorCode::ExtractionEigenvalueNegative);
                }
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
        method => rotate(method, &loadings, u64::from(params.iterations), p, &mut notes, || at(rotation_point))?,
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
    // The notes in decision 717's order.
    let notes = [
        FactorCode::CommunalityAboveOne,
        FactorCode::ExtractionEigenvalueNegative,
        FactorCode::OneFactorNotRotated,
        FactorCode::RotationNotConverged,
        FactorCode::RotationSingular,
        FactorCode::EigenvaluesNotDistinct,
    ]
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

/// Loadings v_j r_j over the m largest eigenvalues, r_j each root.
fn loadings_from(vectors: &[Vec<f64>], roots: &[f64]) -> Vec<Vec<f64>> {
    let p = vectors.len();
    (0..p).map(|i| roots.iter().enumerate().map(|(j, r)| vectors[j][i] * r).collect()).collect()
}

/// Principal components: loadings v_j √λ_j for the m largest eigenvalues, each
/// above 0, or the refusal (decision 717 keeps it).
fn component_loadings<B>(values: &[f64], vectors: &[Vec<f64>], m: usize) -> Result<Vec<Vec<f64>>, Halt<B>> {
    if values[..m].iter().any(|v| !(*v > 0.0)) {
        return Err(refuse(FactorCode::ExtractionEigenvalueNotPositive));
    }
    let roots: Vec<f64> = values[..m].iter().map(|v| v.sqrt()).collect();
    Ok(loadings_from(vectors, &roots))
}

/// Principal axis: loadings v_j √|λ_j| for the m largest eigenvalues by signed
/// value, as SPSS takes them (IBM, pp. 1-2: Γ = diag(|γ_1|, …, |γ_m|); decision
/// 717), and whether any of them was below 0.
fn axis_loadings(values: &[f64], vectors: &[Vec<f64>], m: usize) -> (Vec<Vec<f64>>, bool) {
    let negative = values[..m].iter().any(|v| *v < 0.0);
    let roots: Vec<f64> = values[..m].iter().map(|v| v.abs().sqrt()).collect();
    (loadings_from(vectors, &roots), negative)
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
        per_item.push(ItemAdequacy { item: items[i], msa: adequacy(ri, qi) });
    }
    SamplingAdequacy { overall: adequacy(r2, q2), items: per_item }
}

/// Σ r² / (Σ r² + Σ q²); a 0/0, where no correlation is counted at all, is
/// `no_correlation` (decision 714).
fn adequacy(r2: f64, q2: f64) -> Stat {
    if r2 + q2 == 0.0 {
        Stat::NotComputable(FactorCode::NoCorrelation)
    } else {
        stat(r2 / (r2 + q2))
    }
}

/// The cosine and sine of P = ¼ atan2(X, Y), Kaiser's angle (IBM, p. 10), with
/// no atan2, sin or cos (decision 699's rule): `None` at X = Y = 0, of either
/// sign, which gives no rotation (atan2 would give a 45° turn at (±0, −0)).
/// With g = max(|X|, |Y|) nothing overflows or underflows; c and s are the
/// cosine and sine of 4P; the half angle is taken twice, each branch dividing
/// only by a number of size at least √2 (decision 715).
fn quarter_angle(x: f64, y: f64) -> Option<(f64, f64)> {
    if x == 0.0 && y == 0.0 {
        return None;
    }
    let g = x.abs().max(y.abs());
    let (xs, ys) = (x / g, y / g);
    let r = (xs * xs + ys * ys).sqrt();
    let (c, s) = (ys / r, xs / r);
    let (cos_2p, sin_2p) = if c >= 0.0 {
        let cos_2p = ((1.0 + c) / 2.0).sqrt();
        (cos_2p, s / (2.0 * cos_2p))
    } else {
        // 4P in (π/2, π] or [−π, −π/2): sin 2P takes s's sign, or X's sign bit
        // when s is 0, as atan2 gives ±π.
        let negative = if s == 0.0 { x.is_sign_negative() } else { s < 0.0 };
        let size = ((1.0 - c) / 2.0).sqrt();
        let sin_2p = if negative { -size } else { size };
        (s / (2.0 * sin_2p), sin_2p)
    };
    let cos_p = ((1.0 + cos_2p) / 2.0).sqrt();
    Some((cos_p, sin_2p / (2.0 * cos_p)))
}

/// Varimax's criterion SV of the normalised loadings Z (IBM, p. 10, not divided
/// by p²; decision 715): Σ_j (p Σ_i z_ij⁴ − (Σ_i z_ij²)²), every power a product.
fn varimax_criterion(z: &[Vec<f64>], m: usize) -> f64 {
    let p = z.len() as f64;
    (0..m)
        .map(|j| {
            let (mut squares, mut fourths) = (0.0, 0.0);
            for row in z {
                let q = row[j] * row[j];
                squares += q;
                fourths += q * q;
            }
            p * fourths - squares * squares
        })
        .sum()
}

/// A varimax rotation as SPSS gives it: the loadings, reflected and sorted
/// (steps f and g), the transformation T with them, and the cycles used.
struct Varimax {
    loadings: Vec<Vec<f64>>,
    t: Vec<Vec<f64>>,
    cycles: u64,
}

/// Varimax as SPSS computes it (IBM, pp. 10-12; decision 715). Kaiser's row
/// normalisation, each row over w_i = √Σ_j λ_ij², a row with w_i below 1e-15 left
/// unscaled; T the identity; SV before the first cycle. A cycle rotates every
/// pair of factors (j, k), j < k, by Kaiser's angle P = ¼ atan2(X, Y), skipping a
/// pair whose |sin P| is at most 1e-15, Z's and T's columns together; after
/// each cycle SV again, and the rotation stops at the first cycle whose |ΔSV| is
/// at most 1e-4. `None` when `limit` cycles (the spec's iterations, SPSS's
/// ITERATE) pass without it. Then each row times w_i, each factor whose sum is
/// below 0 reversed by `stats_linalg`'s sign rule (step f), and the factors
/// sorted by their sums of squares, largest first, ties in order (step g), T's
/// columns with them. `tick` is the rotation's check point, called every cycle.
fn varimax<B>(
    loadings: &[Vec<f64>],
    limit: u64,
    mut tick: impl FnMut() -> Result<(), Halt<B>>,
) -> Result<Option<Varimax>, Halt<B>> {
    let p = loadings.len();
    let m = loadings[0].len();
    let n = p as f64;
    let w: Vec<f64> = loadings
        .iter()
        .map(|row| {
            let w = row.iter().map(|x| x * x).sum::<f64>().sqrt();
            if w < ROW_NORM_FLOOR {
                1.0
            } else {
                w
            }
        })
        .collect();
    let mut z: Vec<Vec<f64>> = loadings.iter().zip(&w).map(|(row, wi)| row.iter().map(|v| v / wi).collect()).collect();
    let mut t: Vec<Vec<f64>> = (0..m).map(|i| (0..m).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    let mut sv = varimax_criterion(&z, m);
    for cycle in 1..=limit {
        tick()?;
        for j in 0..m - 1 {
            for k in j + 1..m {
                let (mut a, mut b, mut c, mut d) = (0.0, 0.0, 0.0, 0.0);
                for row in &z {
                    let u = row[j] * row[j] - row[k] * row[k];
                    let v = 2.0 * row[j] * row[k];
                    a += u;
                    b += v;
                    c += u * u - v * v;
                    d += 2.0 * u * v;
                }
                let x = d - 2.0 * a * b / n;
                let y = c - (a * a - b * b) / n;
                let Some((cos_p, sin_p)) = quarter_angle(x, y) else { continue };
                if sin_p.abs() <= ROTATION_SINE_FLOOR {
                    continue;
                }
                for row in z.iter_mut().chain(t.iter_mut()) {
                    let (lj, lk) = (row[j], row[k]);
                    row[j] = lj * cos_p + lk * sin_p;
                    row[k] = -lj * sin_p + lk * cos_p;
                }
            }
        }
        let next = varimax_criterion(&z, m);
        if (next - sv).abs() <= VARIMAX_CRITERION {
            let mut rotated: Vec<Vec<f64>> = z.into_iter().zip(&w).map(|(row, wi)| row.into_iter().map(|v| v * wi).collect()).collect();
            // Step f: reflect each factor whose loadings sum below 0, with T.
            let flip: Vec<bool> = transpose(&rotated).iter().map(|col| sign_reversed(col)).collect();
            for row in rotated.iter_mut().chain(t.iter_mut()) {
                for (x, f) in row.iter_mut().zip(&flip) {
                    if *f {
                        *x = -*x;
                    }
                }
            }
            // Step g: largest sum of squares first, ties in order, with T.
            let sums = column_sums_of_squares(&rotated);
            let mut order: Vec<usize> = (0..m).collect();
            order.sort_by(|a, b| sums[*b].total_cmp(&sums[*a]));
            let reorder = |rows: Vec<Vec<f64>>| -> Vec<Vec<f64>> {
                rows.into_iter().map(|row| order.iter().map(|&k| row[k]).collect()).collect()
            };
            return Ok(Some(Varimax { loadings: reorder(rotated), t: reorder(t), cycles: cycle }));
        }
        sv = next;
    }
    Ok(None)
}

/// The rotation `method` of the extracted loadings, within `limit` cycles. A
/// rotation that cannot be given is a note, never a refusal: the run keeps
/// everything else (decision 717). `None` with `rotation_not_converged` when
/// varimax does not meet its criterion within the limit (promax starts from it),
/// or with `rotation_singular` when promax gives no rotation: a varimax column
/// at most 1e-7 times the longest (decision 728), or its least squares or an
/// inverse that cannot be computed.
fn rotate<B>(
    method: Rotation,
    loadings: &[Vec<f64>],
    limit: u64,
    p: usize,
    notes: &mut BTreeSet<FactorCode>,
    tick: impl FnMut() -> Result<(), Halt<B>>,
) -> Result<Option<RotatedFactors>, Halt<B>> {
    Ok(match varimax(loadings, limit, tick)? {
        None => {
            notes.insert(FactorCode::RotationNotConverged);
            None
        }
        Some(v) if method == Rotation::Varimax => Some(varimax_result(v, p)),
        Some(v) => {
            let rotated = promax_of(&v.loadings, v.cycles);
            if rotated.is_none() {
                notes.insert(FactorCode::RotationSingular);
            }
            rotated
        }
    })
}

/// Varimax's result: the loadings, T, each factor's sums of squares with its
/// percents, and the cycles as its iterations. A rotated row is the extracted row
/// times T, in the final column order.
fn varimax_result(v: Varimax, p: usize) -> RotatedFactors {
    RotatedFactors {
        method: Rotation::Varimax,
        iterations: v.cycles,
        sums: Some(sums_of(&column_sums_of_squares(&v.loadings), p)),
        loadings: stats_of(&v.loadings),
        transformation: Some(stats_of(&v.t)),
        structure: None,
        factor_correlations: None,
    }
}

/// Promax as SPSS computes it (IBM, pp. 18-19; decision 716), at power 4, from
/// varimax's loadings Λ_R, already reflected and sorted. The target P from the
/// row-normalised loadings, p_ij = λ*_ij |λ*_ij|³ with λ*_ij = λ_ij / h_i (a row
/// with h_i below 1e-15 left as it is); L the least squares of Λ_R L = P, no
/// intercept; D = diag(LᵀL)^(-1/2) and Q = L D, columns of length 1; C =
/// diag((QᵀQ)⁻¹)^(-1/2); the pattern Λ_R Q C⁻¹, the factor correlations Φ = C
/// (QᵀQ)⁻¹ C and the structure the pattern times Φ. Neither reflected nor sorted
/// after the fit. `None` when a column of Λ_R is at most 1e-7 times the longest
/// (decision 728), or when the least squares, a zero column of L or the
/// Cholesky factor refuses: no rotation, the note rotation_singular. Its
/// iterations are varimax's cycles.
fn promax_of(a: &[Vec<f64>], cycles: u64) -> Option<RotatedFactors> {
    let m = a.first().map_or(0, Vec::len);
    // Decision 728 (the review of 9B-2, M1): a column of Λ_R no longer than
    // LEAST_SQUARES_RANK_TOLERANCE times the longest is rounding noise beside
    // the others, which least_squares' own test (each column against its own
    // starting norm) lets through; EFAtools' SPSS route refuses it. No rotation.
    let column_lengths: Vec<f64> = (0..m).map(|j| a.iter().map(|row| row[j] * row[j]).sum::<f64>().sqrt()).collect();
    let longest = column_lengths.iter().fold(0.0f64, |x, y| x.max(*y));
    if column_lengths.iter().any(|x| !(*x > LEAST_SQUARES_RANK_TOLERANCE * longest)) {
        return None;
    }
    let target: Vec<Vec<f64>> = a
        .iter()
        .map(|row| {
            let h = row.iter().map(|x| x * x).sum::<f64>().sqrt();
            let h = if h < ROW_NORM_FLOOR { 1.0 } else { h };
            row.iter()
                .map(|x| {
                    let s = x / h;
                    s * s.abs() * s.abs() * s.abs()
                })
                .collect()
        })
        .collect();
    let l = least_squares(a, &target)?;
    let lengths: Vec<f64> = (0..m).map(|j| l.iter().map(|row| row[j] * row[j]).sum::<f64>().sqrt()).collect();
    if lengths.iter().any(|x| !(*x > 0.0) || !x.is_finite()) {
        return None;
    }
    let q: Vec<Vec<f64>> = l.iter().map(|row| row.iter().zip(&lengths).map(|(x, d)| x / d).collect()).collect();
    let qtq_inverse = spd_inverse(&cholesky(&multiply(&transpose(&q), &q)).ok()?);
    let c: Vec<f64> = (0..m).map(|j| 1.0 / qtq_inverse[j][j].sqrt()).collect();
    // Q C⁻¹: each column of Q over its c_j.
    let q_over_c: Vec<Vec<f64>> = q.iter().map(|row| row.iter().zip(&c).map(|(x, cj)| x / cj).collect()).collect();
    let pattern = multiply(a, &q_over_c);
    // Decision 728 (m2): Φ on the upper triangle, its diagonal included, then
    // mirrored, so it is exactly symmetric and the structure below is exactly
    // the pattern times it. The diagonal stays as computed, as SPSS's.
    let mut phi = vec![vec![0.0; m]; m];
    for j in 0..m {
        for k in j..m {
            let v = c[j] * qtq_inverse[j][k] * c[k];
            phi[j][k] = v;
            phi[k][j] = v;
        }
    }
    let structure = multiply(&pattern, &phi);
    Some(RotatedFactors {
        method: Rotation::Promax,
        iterations: cycles,
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

#[cfg(test)]
mod spss_tier;

/// What no dataset reaches, or a bound decision 700 states, with crafted
/// numbers.
#[cfg(test)]
mod tests {
    use super::*;

    /// Seeded standard normal columns, `p` of `n` rows: splitmix64 and the
    /// Box-Muller transform. Formula data are singular at p + 1 rows; these are not.
    fn normal_table(p: usize, n: usize, seed: u64) -> Dataset {
        let mut state = seed;
        let mut uniform = || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut normal = || {
            let (u1, u2) = (1.0 - uniform(), uniform());
            (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
        };
        let headers: Vec<String> = (1..=p).map(|j| format!("x{j}")).collect();
        let rows: Vec<Vec<String>> = (0..n).map(|_| (0..p).map(|_| format!("{:?}", normal())).collect()).collect();
        crate::stats_l1::dataset::build_dataset(
            crate::stats_l1::dataset::RawTable::from_rows(headers, rows),
            crate::stats_l1::dataset::DataFingerprint::of_bytes(
                format!("normal {p} {n} {seed}").as_bytes(),
                "bartlett",
                crate::stats_l1::dataset::TableRef::FirstTable,
            ),
        )
        .expect("a table")
    }

    /// Decision 700's premise, through `run` (decision 714, m3 of the review of
    /// 9B-1): at the fewest rows a positive-definite R allows, n = p + 1, Bartlett's
    /// multiplier n − 1 − (2p + 5)/6 = (4p − 5)/6 is above 0, so χ² is a value above
    /// 0 and p a value in [0, 1], for p of 3, 10, 50 and the most items, 128.
    #[test]
    fn bartletts_multiplier_is_above_0_whenever_the_rows_are_more_than_the_items() {
        for (p, seed) in [(3usize, 20260714u64), (10, 20260715), (50, 20260716), (128, 20260717)] {
            let d = normal_table(p, p + 1, seed);
            let params = FactorParams {
                items: (1..=p as u32).map(|c| Alias(crate::stats_l1::dataset::ColumnAlias(c))).collect(),
                extraction: Extraction::PrincipalComponents,
                factors: Some(1),
                rotation: Rotation::None,
                iterations: 25,
                convergence: 0.001,
                missing_codes: Vec::new(),
            };
            let r = compute(&d, &params).unwrap_or_else(|e| panic!("p {p}: refused {:?}", e.code));
            match (r.bartlett.chi_square, r.bartlett.p) {
                (Stat::Value(chi), Stat::Value(pv)) => {
                    assert!(chi > 0.0 && (0.0..=1.0).contains(&pv), "p {p}: χ² {chi}, p {pv}");
                }
                other => panic!("p {p}: Bartlett's test not given: {other:?}"),
            }
        }
    }

    /// Decision 715's cosine and sine of P = ¼ atan2(X, Y), against atan2, sin and
    /// cos over at least 10,000 (X, Y): both axes, signed zeros, tiny angles and
    /// |X|, |Y| from 1e-300 to 1e300. Gates 4.5e-16 on cos P and sin P, 1e-15
    /// relative on sin P, and the same skip decision (|sin P| ≤ 1e-15) at every
    /// point. X = Y = 0 gives no rotation, where atan2 gives a 45° turn at
    /// (±0, −0): checked apart.
    #[test]
    fn the_quarter_angle_matches_atan2_sin_and_cos() {
        let mut points: Vec<(f64, f64)> = Vec::new();
        let edge = [0.0, -0.0, 1.0, -1.0, 1e-300, -1e-300, 1e300, -1e300, f64::MIN_POSITIVE, -f64::MIN_POSITIVE, 2.5, -0.75];
        for &x in &edge {
            for &y in &edge {
                points.push((x, y));
            }
        }
        // Magnitudes 1e-300 to 1e300, every sign.
        for ex in (-300..=300).step_by(25) {
            for ey in (-300..=300).step_by(25) {
                let (mx, my) = (10f64.powi(ex) * 1.37, 10f64.powi(ey) * 0.83);
                for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                    points.push((sx * mx, sy * my));
                }
            }
        }
        // The circle, at several scales.
        for k in 0..4000 {
            let theta = -std::f64::consts::PI + 2.0 * std::f64::consts::PI * (k as f64 + 0.5) / 4000.0;
            for scale in [1e-200, 1.0, 1e150] {
                points.push((scale * theta.sin(), scale * theta.cos()));
            }
        }
        // Tiny angles either side of both axes, near the skip's edge too.
        for e in 1..=320 {
            let t = 10f64.powi(-e);
            for (x, y) in [(t, 1.0), (-t, 1.0), (t, -1.0), (-t, -1.0), (1.0, t), (1.0, -t), (4.0e-15, 1.0), (-4.1e-15, 1.0)] {
                points.push((x, y));
            }
        }
        assert!(points.len() >= 10_000, "{}", points.len());
        let (mut worst_c, mut worst_s, mut worst_rel) = (0.0f64, 0.0f64, 0.0f64);
        for &(x, y) in &points {
            let got = quarter_angle(x, y);
            if x == 0.0 && y == 0.0 {
                assert_eq!(got, None, "({x:?}, {y:?}): no rotation");
                continue;
            }
            let p = 0.25 * x.atan2(y);
            let (wc, ws) = (p.cos(), p.sin());
            let (gc, gs) = got.unwrap_or_else(|| panic!("({x:e}, {y:e}): no angle"));
            assert_eq!(gs.abs() > ROTATION_SINE_FLOOR, ws.abs() > ROTATION_SINE_FLOOR, "({x:e}, {y:e}): the skip, sin {gs:e} against {ws:e}");
            worst_c = worst_c.max((gc - wc).abs());
            worst_s = worst_s.max((gs - ws).abs());
            if ws != 0.0 {
                worst_rel = worst_rel.max(((gs - ws) / ws).abs());
            }
        }
        assert!(worst_c <= 4.5e-16 && worst_s <= 4.5e-16 && worst_rel <= 1e-15, "cos {worst_c:e}, sin {worst_s:e}, sin relative {worst_rel:e} over {} points", points.len());
    }

    /// Decision 717: a varimax with a zero column leaves promax's least squares
    /// rank deficient, so promax gives no rotation (the note rotation_singular),
    /// never a refusal and never a NaN.
    #[test]
    fn promax_from_a_varimax_with_a_zero_column_gives_no_rotation() {
        let a = vec![vec![0.8, 0.0], vec![0.7, 0.0], vec![0.6, 0.0], vec![0.5, 0.0]];
        assert!(promax_of(&a, 3).is_none());
    }

    /// Decision 717, through the run's dispatch: loadings with a zero column keep it
    /// under varimax (with z_k = 0, B = D = 0 and Y ≥ 0, so P = 0), so promax's
    /// least squares refuses; the rotation is not given and the note
    /// rotation_singular says so, never a refusal of the run. Varimax itself is
    /// given.
    #[test]
    fn a_rotation_promax_cannot_give_is_a_note_not_a_refusal() {
        let loadings = vec![vec![0.8, 0.0], vec![0.7, 0.0], vec![0.6, 0.0], vec![0.5, 0.0]];
        let mut notes = BTreeSet::new();
        let got = rotate::<Infallible>(Rotation::Promax, &loadings, 25, 4, &mut notes, || Ok(()));
        assert!(matches!(got, Ok(None)), "{got:?}");
        assert_eq!(notes.into_iter().collect::<Vec<_>>(), vec![FactorCode::RotationSingular]);
        let mut notes = BTreeSet::new();
        let got = rotate::<Infallible>(Rotation::Varimax, &loadings, 25, 4, &mut notes, || Ok(()));
        assert!(matches!(got, Ok(Some(_))) && notes.is_empty(), "{got:?} {notes:?}");
    }

    /// Decision 728 (the review of 9B-2, M1): the review's d337, the extracted
    /// loadings of 4 items and 3 principal axis factors, whose third column is
    /// rounding noise beside columns of about 0.5. least_squares' own rank test
    /// compares each column with its own starting norm, so it passed, and promax
    /// gave the noise factor a Φ of 0.794 and 0.858. A column at most
    /// LEAST_SQUARES_RANK_TOLERANCE times the longest is now no rotation, with the
    /// note rotation_singular, as EFAtools' SPSS route refuses it.
    #[test]
    fn promax_of_a_column_of_rounding_noise_is_rotation_singular() {
        let loadings = vec![
            vec![0.65402814057069014, 0.037868947412225004, 8.5074847629087663e-32],
            vec![0.40058765451241041, -0.40111010693582511, -7.9150279320799626e-32],
            vec![0.30489011194376398, 0.44577503288242454, -7.8452976556733472e-32],
            vec![-1.4669968587295134e-19, 2.6148518817658068e-20, 1.0349614773508631e-16],
        ];
        let mut notes = BTreeSet::new();
        let got = rotate::<Infallible>(Rotation::Promax, &loadings, 25, 4, &mut notes, || Ok(()));
        assert!(matches!(got, Ok(None)), "{got:?}");
        assert_eq!(notes.into_iter().collect::<Vec<_>>(), vec![FactorCode::RotationSingular]);
    }

    #[test]
    fn a_nan_is_too_small_and_an_infinity_overflow() {
        assert_eq!(stat(f64::NAN), Stat::NotComputable(FactorCode::TooSmall));
        assert_eq!(stat(f64::INFINITY), Stat::NotComputable(FactorCode::Overflow));
        assert_eq!(stat(-0.5), Stat::Value(-0.5));
    }
}
