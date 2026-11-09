//! **Crosstabs and chi-square as an analysis of the user's own data
//! (Research OS, Phase 8X-1, decisions 575 to 578):
//! `stats.analyze.crosstabs`.**
//!
//! Not a reproduction of a reported result: that is Level 1's
//! `stats.verify.chi_square_independence`. This module counts how often each
//! pair of levels of two columns occurs together and computes, from the
//! counts alone:
//!
//! * each cell's observed count, the row totals, the column totals and n;
//!   each cell's expected count, (row total × column total) / n, the product
//!   of two whole numbers divided once; its row, column and total
//!   proportion; its residual, O − E; its Pearson residual, (O − E) / √E;
//!   and its adjusted standardized residual, (O − E) /
//!   √(E (1 − row total / n)(1 − column total / n)), A. Agresti,
//!   Categorical Data Analysis, 3rd ed., Wiley, 2013, section 2.4.5;
//! * Pearson's chi-square, K. Pearson, "On the criterion that a given system
//!   of deviations from the probable…", Philosophical Magazine 50(302),
//!   157–175, 1900, <https://doi.org/10.1080/14786440009463897>, with df
//!   (r − 1)(c − 1), through `stats_verify::chi_square_independence`, the
//!   one implementation (its p, statrs's 1 − cdf, is never read);
//! * the likelihood ratio, G² = 2 Σ O ln(O / E) over the cells with O above
//!   0, clamped at 0 from below (rounding can leave it a little below 0 on
//!   a near-independent table), with the same df;
//! * for a 2 x 2 table only, the continuity-corrected chi-square, F. Yates,
//!   "Contingency tables involving small numbers and the χ² test",
//!   Supplement to the JRSS 1(2), 217–235, 1934,
//!   <https://doi.org/10.2307/2983604>, through
//!   `stats_verify::chi_square_2x2_yates`, df 1; and Fisher's exact test,
//!   `stats_dist::fisher_2x2`, its two-sided p and its two one-sided p,
//!   less and greater;
//! * phi: for a 2 x 2 table signed, (n11 n22 − n12 n21) /
//!   √(r1 r2 c1 c2); for a larger table √(χ² / n); Cramér's V,
//!   √(χ² / (n (min(r, c) − 1))), H. Cramér, Mathematical Methods of
//!   Statistics, Princeton University Press, 1946, section 21.9; and the
//!   contingency coefficient, √(χ² / (χ² + n)).
//!
//! Every p of a chi-square comes from `stats_dist::chi_square_upper`. No p
//! is called anything: the result says what the counts are, never whether a
//! test holds.
//!
//! **The data (decision 575).** Each column's levels are
//! `stats_ttest::group_levels`', in code-point order, kept where an included
//! row has them. The missing codes apply by value to both columns: a cell
//! that is a number equal to a code is left out, compared as a number before
//! its level code is taken; a text cell never matches a code. A row is left
//! out once, by the first reason found, the rows' column first:
//! `missing_row` or `missing_code` naming the rows' column, then
//! `missing_column` or `missing_code` naming the columns' column. Each
//! column needs 2 to `MAX_GROUP_LEVELS` levels among the included rows, so a
//! table has at most 2,500 cells and 2,401 df. A score is never a column.
//!
//! **The levels' order.** Code-point order lists a column of numbers 10
//! before 2 and −1 before −2, so the sign of phi and the direction of
//! Fisher's one-sided tests follow it (the limitation `code_point_order`).
//!
//! **Pure** (`tests/contracts_are_pure.rs`): no file, network, process,
//! clock or model. The run's one check point is the caller's, before the
//! arithmetic.
//!
//! **Privacy.** [`CrosstabsOutcome`] holds the level labels, for the user's
//! own screen and export; their `Debug` prints them as counts. The cells
//! never leave [`Split`] and [`Ready`], which derive nothing.

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
    input_ref, is_prepared_column, preparation_holds, suspect_codes_unless_score, Alias,
};
use crate::stats_dist::{chi_square_upper, fisher_2x2};
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_ttest::{column, group_levels, MAX_GROUP_LEVELS};
use crate::stats_verify::{chi_square_2x2_yates, chi_square_independence};

pub mod export;

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.crosstabs";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// The count below which a cell's expected count is counted and noted.
pub const EXPECTED_FLOOR: u64 = 5;

closed! {
    /// Every refusal, outcome, exclusion, note and limitation of crosstabs.
    pub enum XtabCode in "stats.xtab." {
        /// The spec names another capability.
        UnknownCapability => "unknown_capability",
        /// The spec was written for another version of this capability.
        /// Params `expected`, `found`.
        CapabilityVersionMismatch => "capability_version_mismatch",
        /// The spec does not name exactly one input. Param `count`.
        InputCount => "input_count",
        /// The spec's input is not this dataset.
        InputMismatch => "input_mismatch",
        /// The spec's preparation does not describe this table (decision 417).
        PreparationMismatch => "preparation_mismatch",
        /// A missing code is NaN or an infinity. Param `index`.
        MissingCodeNotFinite => "missing_code_not_finite",
        /// A missing code is listed twice. Param `index`.
        MissingCodeRepeated => "missing_code_repeated",
        /// The missing codes are not ascending; the first out of order.
        /// Param `index`.
        MissingCodesOutOfOrder => "missing_codes_out_of_order",
        /// No column has that alias. Param `column`.
        UnknownColumn => "unknown_column",
        /// The rows and the columns are one column.
        SameColumn => "same_column",
        /// A score is never a column of a table (decision 575). Param
        /// `column`.
        ScoreAsColumn => "score_as_column",
        /// A column has fewer than two levels among the included rows.
        /// Params `column`, `count`.
        TooFewLevels => "too_few_levels",
        /// A column has more levels among the included rows than a table
        /// takes. Params `column`, `count`, `limit`.
        TooManyLevels => "too_many_levels",
        /// A candidate missing code the rule found in a column and the user
        /// did not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Not computable: a sum, product, quotient or root a double cannot
        /// hold.
        Overflow => "overflow",
        /// Not computable: the chi-square's arithmetic refused the table.
        ArithmeticRefused => "arithmetic_refused",
        /// Not computable: the chi-square is not measured at this many
        /// degrees of freedom (above 2,401).
        DistributionOutOfRange => "distribution_out_of_range",
        /// Note: at least one cell's expected count is below 5.
        ExpectedBelowFive => "expected_below_five",
        /// Excluded: the row's cell in the rows' column is empty or a
        /// missing-value token. Param `column`.
        MissingRow => "missing_row",
        /// Excluded: the row's cell in the columns' column is empty or a
        /// missing-value token. Param `column`.
        MissingColumn => "missing_column",
        /// Excluded: the row's cell is a number the user listed. Param
        /// `column`, the column it was found in.
        MissingCode => "missing_code",
        /// Limitation: no third, layer column.
        NoLayers => "no_layers",
        /// Limitation: Fisher's exact test for a 2 x 2 table only.
        FisherTwoByTwoOnly => "fisher_two_by_two_only",
        /// Limitation: no lambda, Goodman and Kruskal's tau, uncertainty
        /// coefficient, linear-by-linear test or ordered measures.
        NoFurtherNominalMeasures => "no_further_nominal_measures",
        /// Limitation: each row counts once; no weights.
        NoWeights => "no_weights",
        /// Limitation: the levels are in code-point order, which sets the
        /// sign of phi and the direction of Fisher's one-sided tests.
        CodePointOrder => "code_point_order",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// Crosstabs' parameters: the column whose levels are the table's rows, the
/// column whose levels are its columns, and the missing codes, ascending,
/// which apply by value to both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CrosstabsParams {
    pub rows: Alias,
    pub columns: Alias,
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
    NotComputable(XtabCode),
}

/// One level of a column, by its index in the table (code-point order among
/// the included rows). The label is the user's own text.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelLabel {
    pub level: u32,
    pub label: String,
}

impl std::fmt::Debug for LevelLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LevelLabel")
            .field("level", &self.level)
            .field("label_chars", &self.label.chars().count())
            .finish()
    }
}

/// One cell of the table: its row level and column level by index, and its
/// numbers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frequency {
    pub row: u32,
    pub column: u32,
    pub observed: u64,
    pub expected: f64,
    /// O / the row's total.
    pub row_proportion: f64,
    /// O / the column's total.
    pub column_proportion: f64,
    /// O / n.
    pub total_proportion: f64,
    /// O − E.
    pub residual: f64,
    /// (O − E) / √E.
    pub pearson_residual: f64,
    /// (O − E) / √(E (1 − row total / n)(1 − column total / n)).
    pub adjusted_residual: f64,
}

/// A chi-square statistic, its df and its upper-tail p.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChiTest {
    pub statistic: f64,
    pub df: f64,
    pub p: f64,
}

/// A chi-square test, or why there is none.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChiOutcome {
    Computed(ChiTest),
    NotComputable(XtabCode),
}

/// Fisher's exact test's three p values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FisherTest {
    pub two_sided: f64,
    /// The first cell this small or smaller.
    pub less: f64,
    /// The first cell this large or larger.
    pub greater: f64,
}

/// Crosstabs' result. Every number is finite.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrosstabsOutcome {
    /// The rows' column's levels, in code-point order.
    pub row_levels: Vec<LevelLabel>,
    /// The columns' column's levels, in code-point order.
    pub column_levels: Vec<LevelLabel>,
    /// Every cell, row by row.
    pub frequencies: Vec<Frequency>,
    pub row_totals: Vec<u64>,
    pub column_totals: Vec<u64>,
    pub n: u64,
    pub pearson: ChiOutcome,
    pub likelihood_ratio: ChiOutcome,
    /// For a 2 x 2 table only.
    pub continuity_corrected: Option<ChiOutcome>,
    /// For a 2 x 2 table only.
    pub fisher: Option<FisherTest>,
    pub phi: Stat,
    pub cramers_v: Stat,
    pub contingency_coefficient: Stat,
    /// How many cells have an expected count below [`EXPECTED_FLOOR`].
    pub expected_below_five: u64,
    /// How many cells the table has.
    pub cell_count: u64,
    /// The smallest expected count.
    pub smallest_expected: f64,
    /// The rows left out, by reason and column; reasons with no row are not
    /// listed.
    pub exclusions: Vec<Exclusion<XtabCode>>,
    /// `expected_below_five`, when it applies.
    pub notes: Vec<XtabCode>,
    pub limitations: Vec<XtabCode>,
}

impl CrosstabsOutcome {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, closed words and strings serialises")
    }
}

/// What every result says it does not give.
const LIMITATIONS: [XtabCode; 5] = [
    XtabCode::NoLayers,
    XtabCode::FisherTwoByTwoOnly,
    XtabCode::NoFurtherNominalMeasures,
    XtabCode::NoWeights,
    XtabCode::CodePointOrder,
];

/* ------------------------------------------------------------------------ *
 *  The one splitting pass
 * ------------------------------------------------------------------------ */

/// The included rows' pairs of levels, each by its index among its whole
/// column's levels, with their counts, and the rows left out. Never leaves
/// this module, and derives nothing.
struct Split {
    pairs: BTreeMap<(u32, u32), u64>,
    /// `missing_row`, `missing_code` (rows), `missing_column`,
    /// `missing_code` (columns).
    excluded: [u64; 4],
}

/// Why a cell leaves its row out, or the level it gives. Derives nothing.
enum Reading {
    Empty,
    Coded,
    Level(String),
}

/// A cell's reading: a number equal to a code is compared as a number,
/// before its level code is taken; a text cell never matches a code.
fn read(cell: &Cell, codes: &[f64]) -> Reading {
    match cell {
        Cell::Missing => Reading::Empty,
        Cell::Number(v) if codes.contains(v) => Reading::Coded,
        _ => Reading::Level(
            cell.level_code()
                .expect("a cell that is not missing has a level code"),
        ),
    }
}

fn split(
    rows: &[Cell],
    columns: &[Cell],
    row_index: &BTreeMap<String, u32>,
    column_index: &BTreeMap<String, u32>,
    codes: &[f64],
) -> Split {
    let mut s = Split {
        pairs: BTreeMap::new(),
        excluded: [0; 4],
    };
    for (r, c) in rows.iter().zip(columns) {
        let r_code = match read(r, codes) {
            Reading::Empty => {
                s.excluded[0] += 1;
                continue;
            }
            Reading::Coded => {
                s.excluded[1] += 1;
                continue;
            }
            Reading::Level(code) => code,
        };
        let c_code = match read(c, codes) {
            Reading::Empty => {
                s.excluded[2] += 1;
                continue;
            }
            Reading::Coded => {
                s.excluded[3] += 1;
                continue;
            }
            Reading::Level(code) => code,
        };
        let i = row_index[r_code.as_str()];
        let j = column_index[c_code.as_str()];
        *s.pairs.entry((i, j)).or_insert(0) += 1;
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
    parameters: CrosstabsParams,
) -> OperationSpec<CrosstabsParams> {
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

fn column_param(alias: Alias) -> ParamValue<XtabCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

struct Checked {
    errors: Vec<Diagnostic<XtabCode>>,
    ready: Option<Ready>,
}

/// The table's levels and counts, once the spec is checked. Holds the
/// labels; derives nothing.
struct Ready {
    row_labels: Vec<String>,
    column_labels: Vec<String>,
    /// observed[i][j], over the kept levels.
    observed: Vec<Vec<u64>>,
    excluded: [u64; 4],
}

/// The kept levels' whole-column indices, in code-point order: those an
/// included row has.
fn kept(pairs: &BTreeMap<(u32, u32), u64>, which: fn(&(u32, u32)) -> u32) -> Vec<u32> {
    let set: BTreeSet<u32> = pairs.keys().map(which).collect();
    set.into_iter().collect()
}

/// Shared by `validate` and `run`: the errors in a fixed order (the missing
/// codes, each column, the same column, a score, then each column's levels
/// among the included rows), and the table when there are none.
fn check(dataset: &Dataset, params: &CrosstabsParams) -> Checked {
    let mut errors = Vec::new();
    let codes = &params.missing_codes;
    for (i, code) in codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(
                Diagnostic::error(XtabCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64),
            );
        }
    }
    for (i, w) in codes.windows(2).enumerate() {
        if w[0] == w[1] {
            errors.push(
                Diagnostic::error(XtabCode::MissingCodeRepeated).int(ParamKey::Index, i as i64 + 1),
            );
        } else if w[0] > w[1] {
            errors.push(
                Diagnostic::error(XtabCode::MissingCodesOutOfOrder)
                    .int(ParamKey::Index, i as i64 + 1),
            );
        }
    }
    let rows = column(dataset, params.rows);
    let columns = column(dataset, params.columns);
    for (alias, found) in [
        (params.rows, rows.is_some()),
        (params.columns, columns.is_some()),
    ] {
        if !found {
            errors.push(
                Diagnostic::error(XtabCode::UnknownColumn)
                    .with(ParamKey::Column, column_param(alias)),
            );
        }
    }
    let (Some((_, r_cells)), Some((_, c_cells))) = (rows, columns) else {
        return Checked {
            errors,
            ready: None,
        };
    };
    if params.rows == params.columns {
        errors.push(Diagnostic::error(XtabCode::SameColumn));
    }
    for alias in [params.rows, params.columns] {
        if is_prepared_column(dataset, alias) {
            errors.push(
                Diagnostic::error(XtabCode::ScoreAsColumn)
                    .with(ParamKey::Column, column_param(alias)),
            );
        }
    }
    if !errors.is_empty() {
        return Checked {
            errors,
            ready: None,
        };
    }
    let labels = |alias: Alias| -> Vec<String> {
        group_levels(dataset, alias)
            .expect("the column exists")
            .into_iter()
            .map(|(label, _)| label)
            .collect()
    };
    let (all_rows, all_columns) = (labels(params.rows), labels(params.columns));
    let index = |labels: &[String]| -> BTreeMap<String, u32> {
        labels
            .iter()
            .enumerate()
            .map(|(i, l)| (l.clone(), i as u32))
            .collect()
    };
    let (row_index, column_index) = (index(&all_rows), index(&all_columns));
    let s = split(r_cells, c_cells, &row_index, &column_index, codes);
    let row_kept = kept(&s.pairs, |p| p.0);
    let column_kept = kept(&s.pairs, |p| p.1);
    for (alias, levels) in [
        (params.rows, row_kept.len()),
        (params.columns, column_kept.len()),
    ] {
        if levels < 2 {
            errors.push(
                Diagnostic::error(XtabCode::TooFewLevels)
                    .with(ParamKey::Column, column_param(alias))
                    .int(ParamKey::Count, levels as i64),
            );
        } else if levels > MAX_GROUP_LEVELS {
            errors.push(
                Diagnostic::error(XtabCode::TooManyLevels)
                    .with(ParamKey::Column, column_param(alias))
                    .int(ParamKey::Count, levels as i64)
                    .int(ParamKey::Limit, MAX_GROUP_LEVELS as i64),
            );
        }
    }
    if !errors.is_empty() {
        return Checked {
            errors,
            ready: None,
        };
    }
    let row_at: BTreeMap<u32, usize> = row_kept.iter().enumerate().map(|(k, i)| (*i, k)).collect();
    let column_at: BTreeMap<u32, usize> =
        column_kept.iter().enumerate().map(|(k, j)| (*j, k)).collect();
    let mut observed = vec![vec![0u64; column_kept.len()]; row_kept.len()];
    for ((i, j), n) in &s.pairs {
        observed[row_at[i]][column_at[j]] = *n;
    }
    let ready = Ready {
        row_labels: row_kept
            .iter()
            .map(|i| all_rows[*i as usize].clone())
            .collect(),
        column_labels: column_kept
            .iter()
            .map(|j| all_columns[*j as usize].clone())
            .collect(),
        observed,
        excluded: s.excluded,
    };
    Checked {
        errors,
        ready: Some(ready),
    }
}

/// Checks the spec against the dataset. Unsupported when it names another
/// capability, version or workspace; invalid on a wrong input or a parameter
/// error; a warning for a candidate missing code in either column that the
/// user did not list. Nothing needs input: the table takes every level.
pub fn validate(
    dataset: &Dataset,
    spec: &OperationSpec<CrosstabsParams>,
) -> ValidationReport<XtabCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID
    {
        unsupported.push(Diagnostic::error(XtabCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(XtabCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(XtabCode::InputMismatch));
            }
        }
        inputs => errors.push(
            Diagnostic::error(XtabCode::InputCount).int(ParamKey::Count, inputs.len() as i64),
        ),
    }
    if !preparation_holds(dataset, spec.preparation.as_ref()) {
        errors.push(Diagnostic::error(XtabCode::PreparationMismatch));
    }
    let params = &spec.parameters;
    errors.extend(check(dataset, params).errors);
    let mut warnings = Vec::new();
    let mut seen = Vec::new();
    for alias in [params.rows, params.columns] {
        if seen.contains(&alias) {
            continue;
        }
        seen.push(alias);
        if let Some((_, cells)) = column(dataset, alias) {
            for s in suspect_codes_unless_score(dataset, alias, cells) {
                if !params.missing_codes.contains(&f64::from(s.code)) {
                    warnings.push(
                        Diagnostic::warning(XtabCode::SuspectMissingCode)
                            .with(ParamKey::Column, column_param(alias))
                            .int(ParamKey::Value, i64::from(s.code))
                            .int(ParamKey::Count, s.cells as i64),
                    );
                }
            }
        }
    }
    let rows = dataset.rows() as u64;
    let estimate = ResourceEstimate {
        rows,
        columns: 2,
        cells: rows.saturating_mul(2),
    };
    ValidationReport::new(unsupported, errors, Vec::new(), warnings, estimate)
}

/* ------------------------------------------------------------------------ *
 *  run
 * ------------------------------------------------------------------------ */

/// Why [`run`] returned no result.
#[derive(Debug, Clone, PartialEq)]
pub enum Halt<B> {
    /// The parameters are wrong for this dataset.
    Refused(Diagnostic<XtabCode>),
    /// The caller's check point stopped the run.
    Stopped { reason: B },
}

/// How many check points a run calls: one, before the arithmetic (decision
/// 575). The run manager's progress counts it.
pub fn check_points() -> usize {
    1
}

/// [`run`] with a check point that never stops.
pub fn compute(
    dataset: &Dataset,
    params: &CrosstabsParams,
) -> Result<CrosstabsOutcome, Diagnostic<XtabCode>> {
    match run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// `v`, or `overflow` when it is infinite or NaN.
fn finite(v: f64) -> Result<f64, XtabCode> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(XtabCode::Overflow)
    }
}

/// A chi-square statistic and df with its p, or why not.
fn chi_test(statistic: f64, df: f64) -> ChiOutcome {
    if !statistic.is_finite() {
        return ChiOutcome::NotComputable(XtabCode::Overflow);
    }
    match chi_square_upper(statistic, df) {
        Some(p) => ChiOutcome::Computed(ChiTest { statistic, df, p }),
        None => ChiOutcome::NotComputable(XtabCode::DistributionOutOfRange),
    }
}

/// Crosstabs. `checkpoint(0)` is called after the parameters are checked
/// and before the arithmetic; a `Break` stops the run there. What a stop
/// means is the caller's.
pub fn run<B>(
    dataset: &Dataset,
    params: &CrosstabsParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<CrosstabsOutcome, Halt<B>> {
    let checked = check(dataset, params);
    if let Some(e) = checked.errors.into_iter().next() {
        return Err(Halt::Refused(e));
    }
    let ready = checked
        .ready
        .expect("no error means the columns and the levels are good");
    if let ControlFlow::Break(reason) = checkpoint(0) {
        return Err(Halt::Stopped { reason });
    }
    let observed = &ready.observed;
    let (r, c) = (observed.len(), observed[0].len());
    let row_totals: Vec<u64> = observed.iter().map(|row| row.iter().sum()).collect();
    let column_totals: Vec<u64> = (0..c).map(|j| observed.iter().map(|row| row[j]).sum()).collect();
    let n: u64 = row_totals.iter().sum();
    let nf = n as f64;
    let mut frequencies = Vec::with_capacity(r * c);
    let mut below_five = 0u64;
    let mut smallest = f64::INFINITY;
    let mut g2 = 0.0f64;
    for (i, row) in observed.iter().enumerate() {
        for (j, &o) in row.iter().enumerate() {
            let (ri, cj) = (row_totals[i], column_totals[j]);
            // The product of two whole numbers, divided once: exact below
            // 2^53, so the count below 5 is decided in whole numbers.
            let product = ri * cj;
            if product < EXPECTED_FLOOR * n {
                below_five += 1;
            }
            let e = product as f64 / nf;
            smallest = smallest.min(e);
            let of = o as f64;
            let residual = of - e;
            let pearson_residual = finite(residual / e.sqrt()).map_err(refuse)?;
            let adjusted = finite(
                residual / (e * (1.0 - ri as f64 / nf) * (1.0 - cj as f64 / nf)).sqrt(),
            )
            .map_err(refuse)?;
            if o > 0 {
                g2 += of * (of / e).ln();
            }
            frequencies.push(Frequency {
                row: i as u32,
                column: j as u32,
                observed: o,
                expected: e,
                row_proportion: of / ri as f64,
                column_proportion: of / cj as f64,
                total_proportion: of / nf,
                residual,
                pearson_residual,
                adjusted_residual: adjusted,
            });
        }
    }
    let table: Vec<Vec<f64>> = observed
        .iter()
        .map(|row| row.iter().map(|o| *o as f64).collect())
        .collect();
    let df = ((r - 1) * (c - 1)) as f64;
    let pearson = match chi_square_independence(&table) {
        Ok(x) => chi_test(x.chi_square, x.df),
        Err(_) => ChiOutcome::NotComputable(XtabCode::ArithmeticRefused),
    };
    let likelihood_ratio = chi_test((2.0 * g2).max(0.0), df);
    let two_by_two = r == 2 && c == 2;
    let continuity_corrected = two_by_two.then(|| match chi_square_2x2_yates(&table) {
        Ok(x) => chi_test(x.chi_square, x.df),
        Err(_) => ChiOutcome::NotComputable(XtabCode::ArithmeticRefused),
    });
    let fisher = two_by_two.then(|| {
        let f = fisher_2x2(observed[0][0], observed[0][1], observed[1][0], observed[1][1]);
        FisherTest {
            two_sided: f.two_sided,
            less: f.less,
            greater: f.greater,
        }
    });
    let (phi, cramers_v, contingency_coefficient) = match pearson {
        ChiOutcome::Computed(t) => {
            let x2 = t.statistic;
            let phi = if two_by_two {
                let ad = (observed[0][0] * observed[1][1]) as f64;
                let bc = (observed[0][1] * observed[1][0]) as f64;
                let margins = ((row_totals[0] * row_totals[1]) as f64
                    * (column_totals[0] * column_totals[1]) as f64)
                    .sqrt();
                finite((ad - bc) / margins).map_err(refuse)?
            } else {
                finite((x2 / nf).sqrt()).map_err(refuse)?
            };
            let k = r.min(c) as f64 - 1.0;
            (
                Stat::Value(phi),
                Stat::Value(finite((x2 / (nf * k)).sqrt()).map_err(refuse)?),
                Stat::Value(finite((x2 / (x2 + nf)).sqrt()).map_err(refuse)?),
            )
        }
        ChiOutcome::NotComputable(code) => (
            Stat::NotComputable(code),
            Stat::NotComputable(code),
            Stat::NotComputable(code),
        ),
    };
    let mut notes = Vec::new();
    if below_five > 0 {
        notes.push(XtabCode::ExpectedBelowFive);
    }
    let level_labels = |labels: &[String]| -> Vec<LevelLabel> {
        labels
            .iter()
            .enumerate()
            .map(|(i, l)| LevelLabel {
                level: i as u32,
                label: l.clone(),
            })
            .collect()
    };
    let exclusions = [
        (XtabCode::MissingRow, params.rows),
        (XtabCode::MissingCode, params.rows),
        (XtabCode::MissingColumn, params.columns),
        (XtabCode::MissingCode, params.columns),
    ]
    .into_iter()
    .zip(ready.excluded)
    .filter(|(_, count)| *count > 0)
    .map(|((code, alias), count)| Exclusion {
        code,
        count,
        params: BTreeMap::from([(ParamKey::Column, column_param(alias))]),
    })
    .collect();
    Ok(CrosstabsOutcome {
        row_levels: level_labels(&ready.row_labels),
        column_levels: level_labels(&ready.column_labels),
        frequencies,
        row_totals,
        column_totals,
        n,
        pearson,
        likelihood_ratio,
        continuity_corrected,
        fisher,
        phi,
        cramers_v,
        contingency_coefficient,
        expected_below_five: below_five,
        cell_count: (r * c) as u64,
        smallest_expected: smallest,
        exclusions,
        notes,
        limitations: LIMITATIONS.to_vec(),
    })
}

fn refuse<B>(code: XtabCode) -> Halt<B> {
    Halt::Refused(Diagnostic::error(code))
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows
/// used are n; with the exclusions they add up to the data rows. The output
/// hash is SHA-256 of [`CrosstabsOutcome::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<CrosstabsParams>,
    dataset: &Dataset,
    result: &CrosstabsOutcome,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<XtabCode> {
    let parameters = serde_json::to_vec(&spec.parameters)
        .expect("parameters of aliases and numbers serialise");
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        inputs: spec.inputs.iter().map(|i| i.content_hash.clone()).collect(),
        parameters_hash: ContentHash::of_bytes(&parameters),
        seed: spec.random_seed,
        rows_read: dataset.rows() as u64,
        rows_used: result.n,
        exclusions: result.exclusions.clone(),
        platform,
        elapsed_ms,
        output_hash: ContentHash::of_bytes(&result.canonical_bytes()),
        reproducibility: Reproducibility::Bitwise,
        preparation: spec.preparation.clone(),
    }
}
