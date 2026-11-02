//! **Cronbach's alpha as an analysis of the user's own data (Research OS,
//! Phase 8I, decisions 367 to 371): `stats.analyze.cronbach_alpha`.**
//!
//! For a scale of two or more numeric items, over the rows in which every
//! item has a value (listwise, as SPSS's RELIABILITY uses them):
//!
//! * Cronbach's alpha, k/(k − 1) · (1 − Σ sᵢ² / s²ₜₒₜₐₗ), with sᵢ² each
//!   item's variance and s²ₜₒₜₐₗ the variance of the rows' totals (L. J.
//!   Cronbach, "Coefficient alpha and the internal structure of tests",
//!   Psychometrika 16(3), 297–334, 1951,
//!   <https://doi.org/10.1007/BF02310555>);
//! * standardized alpha, k · r̄ / (1 + (k − 1) · r̄), with r̄ the mean of the
//!   k(k − 1)/2 inter-item Pearson correlations, and r̄'s minimum and
//!   maximum beside it;
//! * a 95% interval for alpha by Feldt's F, with df1 = n − 1 and
//!   df2 = (n − 1)(k − 1): 1 − (1 − alpha) · F₀.₉₇₅ and
//!   1 − (1 − alpha) · F₀.₀₂₅, F₀.₉₇₅ the F whose upper-tail p is 0.025 and
//!   F₀.₀₂₅ the F whose upper-tail p is 0.975 (L. S. Feldt, "The approximate
//!   sampling distribution of Kuder-Richardson reliability coefficient
//!   twenty", Psychometrika 30(3), 357–370, 1965,
//!   <https://doi.org/10.1007/BF02289499>; L. S. Feldt, D. J. Woodruff and
//!   F. A. Salih, Applied Psychological Measurement 11(1), 93–103, 1987,
//!   <https://doi.org/10.1177/014662168701100107>). IBM's RELIABILITY
//!   documents the same interval as its two-way consistency, average-
//!   measures intraclass correlation's;
//! * per item, in column order: whether the user reversed it, its mean and
//!   SD after reversing, the scale's mean and variance if it were left out
//!   (the total of the others), the corrected item-total correlation (the
//!   item with the total of the others), and alpha if it were left out.
//!
//! A reversed item's value x is min + max − x, the scale's ends the user
//! gives, before any arithmetic; never the observed range. A value of a
//! reversed item outside [min, max] in any row read, ticked codes removed,
//! is refused before any run. Gaply says which items the user reversed,
//! never which they should reverse; a negative alpha carries a note.
//!
//! **One formula, one place (decisions 62, 275 and 368).** Means and
//! variances are `stats_verify::mean` and `sample_variance` on the values
//! ascending; every correlation is `stats_verify::pearson_clamped`'s r, and
//! one within `RHO_ONE_ULPS` units in the last place of ±1 is ±1 (decision
//! 352's test, Level 1's for rho: x and 6 − x give r = −1, not −1 + 1 ulp);
//! Feldt's quantiles are `stats_dist::f_quantile_upper`. Alpha, standardized
//! alpha, Feldt's interval, the item statistics and the reversal are
//! written once, here. The others' total is the row's total minus the item.
//!
//! **The data (decision 367).** A value is a number that is not a ticked
//! missing code. A row is used when every item has one; a row left out is
//! counted once, under the first item in the column order without a value
//! (`missing_value` or `missing_code`, that column its parameter). Text
//! columns and non-finite cells are refused, and candidate missing codes
//! are warned of, as the correlations do.
//!
//! **One choice, one spec (decision 338's rule).** Items and reversed items
//! in the dataset's column order, missing codes ascending, none repeated; a
//! reversed item not among the items, a scale given with nothing reversed
//! or missing with something reversed, and a scale whose min is not below
//! its max or not finite are refused with closed codes, never corrected
//! silently.
//!
//! **Not computable, decided from the values before each call:** the
//! totals constant, their variance within rounding of 0 (at most
//! (k · ε · M)², M the largest row sum of the items' magnitudes), for alpha
//! and its interval; an item constant (standardized
//! alpha, r̄ and its range, and that item's corrected item-total
//! correlation); 1 + (k − 1) · r̄ ≤ 0 (standardized alpha); the others'
//! total constant (that item's corrected item-total correlation and alpha if
//! left out), by the same rounding bound; k = 2 (every alpha if left out); an overflow or underflow
//! `stats_verify` refuses with (by [`RangeFault::of`]); a df beyond
//! `stats_dist`'s measured range (the interval).
//!
//! **Pure** (`tests/contracts_are_pure.rs`): no file, network, process,
//! clock or model. The run's check points are the caller's: one before the
//! arithmetic, one before each item's row and one before each inter-item
//! correlation ([`check_points`]).
//!
//! **Privacy.** [`AlphaOutcome`] holds numbers, aliases and closed codes,
//! and no header, label or cell.
//!
//! **Exports (8I-2, decision 379).** [`export`] writes the result as one
//! long CSV and the manifest as JSON, bytes only, columns by alias.

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
use crate::stats_dist::f_quantile_upper;
use crate::stats_l1::binding::suspect_codes_in;
use crate::stats_l1::compute::RHO_ONE_ULPS;
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_ttest::{column, non_finite_cells};
use crate::stats_verify::{mean, pearson_clamped, sample_variance, RangeFault};

pub mod export;

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.cronbach_alpha";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// The interval's coverage.
pub const CONFIDENCE_LEVEL: f64 = 0.95;
/// The upper-tail p of Feldt's two F quantiles: the lower end's, then the
/// upper end's.
const FELDT_UPPER_P: [f64; 2] = [0.025, 0.975];

closed! {
    /// Every finding, outcome, exclusion, note and limitation of Cronbach's
    /// alpha.
    pub enum AlphaCode in "stats.alpha." {
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
        /// Fewer than two items are chosen. Param `count`.
        TooFewItems => "too_few_items",
        /// An item is named twice. Param `column`.
        ItemRepeated => "item_repeated",
        /// The items are not in the dataset's column order; the first out of
        /// order. Param `column`.
        ItemsOutOfOrder => "items_out_of_order",
        /// A reversed item is named twice. Param `column`.
        ReversedRepeated => "reversed_repeated",
        /// The reversed items are not in the dataset's column order; the
        /// first out of order. Param `column`.
        ReversedOutOfOrder => "reversed_out_of_order",
        /// A reversed item is not one of the items. Param `column`.
        ReversedNotAnItem => "reversed_not_an_item",
        /// The scale's ends are given but no item is reversed.
        ScaleWithoutReversed => "scale_without_reversed",
        /// An item is reversed but the scale's ends are not given.
        ScaleMissing => "scale_missing",
        /// A scale end is NaN or an infinity.
        ScaleNotFinite => "scale_not_finite",
        /// The scale's lowest value is not below its highest.
        ScaleEndsOutOfOrder => "scale_ends_out_of_order",
        /// A missing code is NaN or an infinity. Param `index`.
        MissingCodeNotFinite => "missing_code_not_finite",
        /// A missing code is listed twice. Param `index`.
        MissingCodeRepeated => "missing_code_repeated",
        /// The missing codes are not ascending; the first out of order.
        /// Param `index`.
        MissingCodesOutOfOrder => "missing_codes_out_of_order",
        /// A chosen column holds text. Params `column`, `count`.
        ItemHoldsText => "item_holds_text",
        /// A chosen column holds NaN or an infinity. Params `column`, `count`.
        ItemNonFinite => "item_non_finite",
        /// A candidate missing code the rule found in a chosen column and the
        /// user did not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Refused: a reversed item has values outside the scale's ends, in
        /// the rows read, ticked codes removed; the ends are wrong or a
        /// missing code is not ticked. Params `column`, `count`.
        ReversedOutsideScale => "reversed_outside_scale",
        /// Refused: fewer than 3 rows have a value in every item. Param
        /// `count`.
        TooFewCompleteRows => "too_few_complete_rows",
        /// Not computable: the rows' totals are the same in every row.
        TotalConstant => "total_constant",
        /// Not computable: an item is the same number in every row used, so a
        /// correlation with it is undefined.
        ItemConstant => "item_constant",
        /// Not computable: 1 + (k − 1) · r̄ is 0 or below.
        StandardizedNotDefined => "standardized_not_defined",
        /// Not computable: the total of the other items is the same in every
        /// row used.
        OthersConstant => "others_constant",
        /// Not computable: with two items, leaving one out leaves one item.
        TwoItems => "two_items",
        /// Not computable: the F distribution is not measured at these
        /// degrees of freedom.
        DistributionOutOfRange => "distribution_out_of_range",
        /// Not computable: a sum, square, quotient or root a double cannot
        /// hold.
        Overflow => "overflow",
        /// Not computable: a sum of squares underflowed though the values
        /// differ.
        Underflow => "underflow",
        /// Not computable: the arithmetic refused for a reason it did not
        /// name.
        ArithmeticRefused => "arithmetic_refused",
        /// Excluded: the row's first item without a value has an empty cell
        /// or a missing-value token there. Param `column`.
        MissingValue => "missing_value",
        /// Excluded: the row's first item without a value has a ticked
        /// missing code there. Param `column`.
        MissingCode => "missing_code",
        /// Note: alpha is negative, the items' average covariance is
        /// negative.
        NegativeAlpha => "negative_alpha",
        /// Limitation: alpha assumes the items measure one thing; Gaply does
        /// not check that.
        AssumesOneDimension => "assumes_one_dimension",
        /// Limitation: McDonald's omega and squared multiple correlations are
        /// not shown.
        NoOmega => "no_omega",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// The lowest and highest values the scale allows, for reversing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScaleEnds {
    pub min: f64,
    pub max: f64,
}

/// Cronbach's alpha's parameters: the items in the dataset's column order,
/// the items to reverse-score (a subset, in the same order, none by
/// default), the scale's ends (required when an item is reversed, absent
/// otherwise), and the missing codes ascending, which apply to every item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlphaParams {
    pub items: Vec<Alias>,
    pub reversed: Vec<Alias>,
    pub scale: Option<ScaleEnds>,
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
    NotComputable(AlphaCode),
}

/// Alpha's 95% interval, or why there is none.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Interval {
    Computed { lower: f64, upper: f64 },
    NotComputable(AlphaCode),
}

/// The mean, minimum and maximum of the inter-item correlations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterItem {
    pub mean: Stat,
    pub min: Stat,
    pub max: Stat,
}

/// One item's statistics, after reversing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStats {
    pub item: Alias,
    pub reversed: bool,
    pub mean: Stat,
    pub sd: Stat,
    pub scale_mean_if_deleted: Stat,
    pub scale_variance_if_deleted: Stat,
    pub corrected_item_total: Stat,
    pub alpha_if_deleted: Stat,
}

/// Cronbach's alpha's result. Every number is finite.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlphaOutcome {
    /// The items.
    pub k: u64,
    /// The rows used, n.
    pub rows_used: u64,
    pub alpha: Stat,
    pub interval: Interval,
    pub standardized: Stat,
    pub inter_item: InterItem,
    /// In the items' order.
    pub items: Vec<ItemStats>,
    /// The rows left out, by reason; reasons with no row are not listed.
    pub exclusions: Vec<Exclusion<AlphaCode>>,
    /// `negative_alpha` when alpha is below 0.
    pub notes: Vec<AlphaCode>,
    pub limitations: Vec<AlphaCode>,
}

impl AlphaOutcome {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, aliases and closed words serialises")
    }
}

/// What every result says it did not assess.
const LIMITATIONS: [AlphaCode; 2] = [AlphaCode::AssumesOneDimension, AlphaCode::NoOmega];

/* ------------------------------------------------------------------------ *
 *  The rows
 * ------------------------------------------------------------------------ */

/// One cell of an item, as this method reads it.
enum Entry {
    Value(f64),
    Missing,
    Code,
}

/// The items' entries, column by column, and which rows are used. Never
/// leaves this module.
struct Rows {
    entries: Vec<Vec<Entry>>,
    used: Vec<bool>,
}

impl Rows {
    fn of(columns: &[&[Cell]], codes: &[f64]) -> Rows {
        let entries: Vec<Vec<Entry>> = columns
            .iter()
            .map(|cells| {
                cells
                    .iter()
                    .map(|c| match c {
                        Cell::Number(v) if codes.contains(v) => Entry::Code,
                        Cell::Number(v) => Entry::Value(*v),
                        // Text is refused by validation before rows are read.
                        Cell::Missing | Cell::Text(_) => Entry::Missing,
                    })
                    .collect()
            })
            .collect();
        let rows = entries.first().map_or(0, Vec::len);
        let used = (0..rows)
            .map(|r| entries.iter().all(|e| matches!(e[r], Entry::Value(_))))
            .collect();
        Rows { entries, used }
    }

    fn rows_used(&self) -> u64 {
        self.used.iter().filter(|u| **u).count() as u64
    }

    /// Item `k`'s values in the used rows, reversed when `ends` is given.
    fn values(&self, k: usize, ends: Option<ScaleEnds>) -> Vec<f64> {
        (0..self.used.len())
            .filter(|r| self.used[*r])
            .filter_map(|r| match self.entries[k][r] {
                Entry::Value(v) => Some(reverse(v, ends)),
                _ => None,
            })
            .collect()
    }

    /// The rows left out, each under its first item without a value.
    fn exclusions(&self, items: &[Alias]) -> Vec<Exclusion<AlphaCode>> {
        let mut counts = vec![[0u64; 2]; items.len()];
        for r in 0..self.used.len() {
            if self.used[r] {
                continue;
            }
            for (k, column) in self.entries.iter().enumerate() {
                match column[r] {
                    Entry::Value(_) => continue,
                    Entry::Missing => counts[k][0] += 1,
                    Entry::Code => counts[k][1] += 1,
                }
                break;
            }
        }
        let mut out = Vec::new();
        for (k, c) in counts.iter().enumerate() {
            for (code, count) in [
                (AlphaCode::MissingValue, c[0]),
                (AlphaCode::MissingCode, c[1]),
            ] {
                if count > 0 {
                    out.push(Exclusion {
                        code,
                        count,
                        params: BTreeMap::from([(ParamKey::Column, column_param(items[k]))]),
                    });
                }
            }
        }
        out
    }
}

/// A value reverse-scored on the scale's ends: min + max − x. The one
/// place the reversal is written.
fn reverse(x: f64, ends: Option<ScaleEnds>) -> f64 {
    match ends {
        Some(e) => e.min + e.max - x,
        None => x,
    }
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
    parameters: AlphaParams,
) -> OperationSpec<AlphaParams> {
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

fn column_param(alias: Alias) -> ParamValue<AlphaCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

/// Repeated or out-of-order aliases in `list`, each with its code.
fn order_errors(
    list: &[Alias],
    repeated: AlphaCode,
    out_of_order: AlphaCode,
    errors: &mut Vec<Diagnostic<AlphaCode>>,
) {
    for w in list.windows(2) {
        if w[0] == w[1] {
            errors.push(Diagnostic::error(repeated).with(ParamKey::Column, column_param(w[1])));
        } else if w[0] > w[1] {
            errors.push(Diagnostic::error(out_of_order).with(ParamKey::Column, column_param(w[1])));
        }
    }
}

/// Shared by `validate` and `run`: the errors in a fixed order (missing
/// codes, items, reversed items, the scale, the columns, the reversed
/// values, then the rows), and the items' cells when every parameter is
/// good.
fn check<'d>(
    dataset: &'d Dataset,
    params: &AlphaParams,
) -> (Vec<Diagnostic<AlphaCode>>, Option<Vec<&'d [Cell]>>) {
    let mut errors = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(
                Diagnostic::error(AlphaCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64),
            );
        }
    }
    for (i, w) in params.missing_codes.windows(2).enumerate() {
        if w[0] == w[1] {
            errors.push(
                Diagnostic::error(AlphaCode::MissingCodeRepeated)
                    .int(ParamKey::Index, i as i64 + 1),
            );
        } else if w[0] > w[1] {
            errors.push(
                Diagnostic::error(AlphaCode::MissingCodesOutOfOrder)
                    .int(ParamKey::Index, i as i64 + 1),
            );
        }
    }
    let items = &params.items;
    if items.len() < 2 {
        errors.push(
            Diagnostic::error(AlphaCode::TooFewItems).int(ParamKey::Count, items.len() as i64),
        );
    }
    order_errors(
        items,
        AlphaCode::ItemRepeated,
        AlphaCode::ItemsOutOfOrder,
        &mut errors,
    );
    let reversed = &params.reversed;
    order_errors(
        reversed,
        AlphaCode::ReversedRepeated,
        AlphaCode::ReversedOutOfOrder,
        &mut errors,
    );
    for alias in reversed {
        if !items.contains(alias) {
            errors.push(
                Diagnostic::error(AlphaCode::ReversedNotAnItem)
                    .with(ParamKey::Column, column_param(*alias)),
            );
        }
    }
    match (params.scale, reversed.is_empty()) {
        (Some(_), true) => errors.push(Diagnostic::error(AlphaCode::ScaleWithoutReversed)),
        (None, false) => errors.push(Diagnostic::error(AlphaCode::ScaleMissing)),
        (Some(s), false) if !(s.min.is_finite() && s.max.is_finite()) => {
            errors.push(Diagnostic::error(AlphaCode::ScaleNotFinite))
        }
        (Some(s), false) if s.min >= s.max => {
            errors.push(Diagnostic::error(AlphaCode::ScaleEndsOutOfOrder))
        }
        _ => {}
    }
    let mut cells = Vec::with_capacity(items.len());
    for alias in items {
        match column(dataset, *alias) {
            None => errors.push(
                Diagnostic::error(AlphaCode::UnknownColumn)
                    .with(ParamKey::Column, column_param(*alias)),
            ),
            Some((summary, c)) => {
                let non_finite = non_finite_cells(summary);
                let texts = c.iter().filter(|x| matches!(x, Cell::Text(_))).count() as i64;
                if non_finite > 0 {
                    errors.push(
                        Diagnostic::error(AlphaCode::ItemNonFinite)
                            .with(ParamKey::Column, column_param(*alias))
                            .int(ParamKey::Count, non_finite as i64),
                    );
                } else if texts > 0 {
                    errors.push(
                        Diagnostic::error(AlphaCode::ItemHoldsText)
                            .with(ParamKey::Column, column_param(*alias))
                            .int(ParamKey::Count, texts),
                    );
                }
                cells.push(c);
            }
        }
    }
    if !errors.is_empty() {
        return (errors, None);
    }
    // A reversed item's values outside the scale, in every row read, ticked
    // codes removed (decision 367).
    if let Some(ends) = params.scale {
        for (alias, c) in items.iter().zip(&cells) {
            if !reversed.contains(alias) {
                continue;
            }
            let outside = c
                .iter()
                .filter(|x| match x {
                    Cell::Number(v) => {
                        !params.missing_codes.contains(v) && (*v < ends.min || *v > ends.max)
                    }
                    _ => false,
                })
                .count();
            if outside > 0 {
                errors.push(
                    Diagnostic::error(AlphaCode::ReversedOutsideScale)
                        .with(ParamKey::Column, column_param(*alias))
                        .int(ParamKey::Count, outside as i64),
                );
            }
        }
    }
    if errors.is_empty() {
        let complete = Rows::of(&cells, &params.missing_codes).rows_used();
        if complete < 3 {
            errors.push(
                Diagnostic::error(AlphaCode::TooFewCompleteRows)
                    .int(ParamKey::Count, complete as i64),
            );
        }
    }
    if errors.is_empty() {
        (errors, Some(cells))
    } else {
        (errors, None)
    }
}

/// Checks the spec against the dataset. Unsupported when it names another
/// capability, version or workspace; invalid on a wrong input or a parameter
/// error; a warning for a candidate missing code in an item that the user
/// did not list.
pub fn validate(
    dataset: &Dataset,
    spec: &OperationSpec<AlphaParams>,
) -> ValidationReport<AlphaCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID
    {
        unsupported.push(Diagnostic::error(AlphaCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(AlphaCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(AlphaCode::InputMismatch));
            }
        }
        inputs => errors.push(
            Diagnostic::error(AlphaCode::InputCount).int(ParamKey::Count, inputs.len() as i64),
        ),
    }
    let params = &spec.parameters;
    errors.extend(check(dataset, params).0);
    let mut warnings = Vec::new();
    for alias in &params.items {
        if let Some((_, cells)) = column(dataset, *alias) {
            for s in suspect_codes_in(alias.0, cells) {
                if !params.missing_codes.contains(&f64::from(s.code)) {
                    warnings.push(
                        Diagnostic::warning(AlphaCode::SuspectMissingCode)
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
    /// The parameters are wrong for this dataset.
    Refused(Diagnostic<AlphaCode>),
    /// The caller's check point stopped the run.
    Stopped { reason: B },
}

/// How many check points a run of `params` calls (decision 371): one before
/// the arithmetic, one before each item's row and one before each of the
/// k(k − 1)/2 inter-item correlations. Only the spec is needed.
pub fn check_points(params: &AlphaParams) -> usize {
    let k = params.items.len();
    1 + k + k * k.saturating_sub(1) / 2
}

/// [`run`] with a check point that never stops.
pub fn compute(
    dataset: &Dataset,
    params: &AlphaParams,
) -> Result<AlphaOutcome, Diagnostic<AlphaCode>> {
    match run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// Cronbach's alpha. `checkpoint(0)` is called after the parameters are
/// checked and before the arithmetic, `checkpoint(1..=k)` before each item's
/// row, and `checkpoint(k + 1..)` before each inter-item correlation, in
/// the order (1, 2), (1, 3), …, (k − 1, k); a `Break` stops the run there.
/// What a stop means is the caller's.
pub fn run<B>(
    dataset: &Dataset,
    params: &AlphaParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<AlphaOutcome, Halt<B>> {
    let (errors, cells) = check(dataset, params);
    if let Some(e) = errors.into_iter().next() {
        return Err(Halt::Refused(e));
    }
    let cells = cells.expect("no error means the columns are good");
    if let ControlFlow::Break(reason) = checkpoint(0) {
        return Err(Halt::Stopped { reason });
    }
    let rows = Rows::of(&cells, &params.missing_codes);
    let reversed: Vec<bool> = params
        .items
        .iter()
        .map(|a| params.reversed.contains(a))
        .collect();
    let values: Vec<Vec<f64>> = (0..params.items.len())
        .map(|i| rows.values(i, params.scale.filter(|_| reversed[i])))
        .collect();
    let mut point = 0usize;
    let mut next = || {
        point += 1;
        checkpoint(point)
    };
    let a = arithmetic(&params.items, &reversed, &values, &mut next)?;
    Ok(AlphaOutcome {
        k: params.items.len() as u64,
        rows_used: rows.rows_used(),
        alpha: a.alpha,
        interval: a.interval,
        standardized: a.standardized,
        inter_item: a.inter_item,
        items: a.items,
        exclusions: rows.exclusions(&params.items),
        notes: match a.alpha {
            Stat::Value(v) if v < 0.0 => vec![AlphaCode::NegativeAlpha],
            _ => Vec::new(),
        },
        limitations: LIMITATIONS.to_vec(),
    })
}

/// The numbers of a run.
struct Arithmetic {
    alpha: Stat,
    interval: Interval,
    standardized: Stat,
    inter_item: InterItem,
    items: Vec<ItemStats>,
}

/// Alpha's formulas over each item's `values` in the used rows (already
/// reversed), calling `next` before each item's row and each inter-item
/// correlation.
fn arithmetic<B>(
    items: &[Alias],
    reversed: &[bool],
    values: &[Vec<f64>],
    next: &mut impl FnMut() -> ControlFlow<B>,
) -> Result<Arithmetic, Halt<B>> {
    let k = values.len();
    let n = values[0].len();
    let kf = k as f64;
    // The rows' totals, the items added in column order.
    let totals: Vec<f64> = (0..n)
        .map(|r| values.iter().fold(0.0, |sum, v| sum + v[r]))
        .collect();
    // Each row's sum of its values' magnitudes: the scale of the rounding in
    // its total.
    let abs_totals: Vec<f64> = (0..n)
        .map(|r| values.iter().fold(0.0, |sum, v| sum + v[r].abs()))
        .collect();
    let variances: Vec<Result<f64, AlphaCode>> = values.iter().map(|v| moments(v).1).collect();
    let constant_item: Vec<bool> = values.iter().map(|v| constant(v)).collect();

    let total_var = moments(&totals).1;
    let alpha = if negligible(&total_var, k, largest(&abs_totals)) {
        Stat::NotComputable(AlphaCode::TotalConstant)
    } else {
        match (sum_of(variances.iter()), total_var) {
            (Ok(sum), Ok(total)) => finite((kf / (kf - 1.0)) * (1.0 - sum / total)),
            (Err(c), _) | (_, Err(c)) => Stat::NotComputable(c),
        }
    };
    let interval = match alpha {
        Stat::Value(a) => feldt(a, n, k),
        Stat::NotComputable(c) => Interval::NotComputable(c),
    };

    let mut rows = Vec::with_capacity(k);
    for i in 0..k {
        if let ControlFlow::Break(reason) = next() {
            return Err(Halt::Stopped { reason });
        }
        let others: Vec<f64> = totals.iter().zip(&values[i]).map(|(t, x)| t - x).collect();
        let (mean_i, var_i) = moments(&values[i]);
        let (mean_o, var_o) = moments(&others);
        let others_magnitude = largest(
            &abs_totals
                .iter()
                .zip(&values[i])
                .map(|(t, x)| t - x.abs())
                .collect::<Vec<_>>(),
        );
        let others_constant = negligible(&var_o, k, others_magnitude);
        let corrected_item_total = if constant_item[i] {
            Stat::NotComputable(AlphaCode::ItemConstant)
        } else if others_constant {
            Stat::NotComputable(AlphaCode::OthersConstant)
        } else {
            correlation(&values[i], &others)
        };
        let alpha_if_deleted = if k == 2 {
            Stat::NotComputable(AlphaCode::TwoItems)
        } else if others_constant {
            Stat::NotComputable(AlphaCode::OthersConstant)
        } else {
            let rest = variances
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, v)| v);
            match (sum_of(rest), var_o) {
                (Ok(sum), Ok(v)) => {
                    let k1 = kf - 1.0;
                    finite((k1 / (k1 - 1.0)) * (1.0 - sum / v))
                }
                (Err(c), _) | (_, Err(c)) => Stat::NotComputable(c),
            }
        };
        rows.push(ItemStats {
            item: items[i],
            reversed: reversed[i],
            mean: mean_i,
            sd: match var_i {
                Ok(v) => finite(v.sqrt()),
                Err(c) => Stat::NotComputable(c),
            },
            scale_mean_if_deleted: mean_o,
            scale_variance_if_deleted: match var_o {
                Ok(v) => finite(v),
                Err(c) => Stat::NotComputable(c),
            },
            corrected_item_total,
            alpha_if_deleted,
        });
    }

    // The inter-item correlations, (1, 2), (1, 3), …, (k − 1, k). A check
    // point before each, computed or not, so the count is the spec's.
    let any_constant = constant_item.iter().any(|c| *c);
    let mut rs = Vec::with_capacity(k * (k - 1) / 2);
    let mut fault = None;
    for i in 0..k {
        for j in i + 1..k {
            if let ControlFlow::Break(reason) = next() {
                return Err(Halt::Stopped { reason });
            }
            if any_constant || fault.is_some() {
                continue;
            }
            match correlation(&values[i], &values[j]) {
                Stat::Value(r) => rs.push(r),
                Stat::NotComputable(c) => fault = Some(c),
            }
        }
    }
    let (inter_item, standardized) = if any_constant {
        let nc = Stat::NotComputable(AlphaCode::ItemConstant);
        (
            InterItem {
                mean: nc,
                min: nc,
                max: nc,
            },
            nc,
        )
    } else if let Some(c) = fault {
        let nc = Stat::NotComputable(c);
        (
            InterItem {
                mean: nc,
                min: nc,
                max: nc,
            },
            nc,
        )
    } else {
        rs.sort_by(f64::total_cmp);
        let rbar = mean(&rs);
        let den = 1.0 + (kf - 1.0) * rbar;
        let standardized = if den <= 0.0 {
            Stat::NotComputable(AlphaCode::StandardizedNotDefined)
        } else {
            finite(kf * rbar / den)
        };
        (
            InterItem {
                mean: finite(rbar),
                min: Stat::Value(rs[0]),
                max: Stat::Value(rs[rs.len() - 1]),
            },
            standardized,
        )
    };
    Ok(Arithmetic {
        alpha,
        interval,
        standardized,
        inter_item,
        items: rows,
    })
}

/// Feldt's 95% interval for `alpha` from `n` rows and `k` items: df1 = n − 1,
/// df2 = (n − 1)(k − 1), lower 1 − (1 − alpha) · F(0.025) and upper
/// 1 − (1 − alpha) · F(0.975), F(p) the F whose upper-tail p is p.
fn feldt(alpha: f64, n: usize, k: usize) -> Interval {
    let df1 = (n - 1) as f64;
    let df2 = df1 * (k - 1) as f64;
    let [lower_p, upper_p] = FELDT_UPPER_P;
    match (
        f_quantile_upper(lower_p, df1, df2),
        f_quantile_upper(upper_p, df1, df2),
    ) {
        (Some(f_lower), Some(f_upper)) => {
            let (lower, upper) = (1.0 - (1.0 - alpha) * f_lower, 1.0 - (1.0 - alpha) * f_upper);
            if lower.is_finite() && upper.is_finite() {
                Interval::Computed { lower, upper }
            } else {
                Interval::NotComputable(AlphaCode::Overflow)
            }
        }
        _ => Interval::NotComputable(AlphaCode::DistributionOutOfRange),
    }
}

/// The mean and the sample variance of `v`, each from `stats_verify` on the
/// values ascending.
fn moments(v: &[f64]) -> (Stat, Result<f64, AlphaCode>) {
    let mut sorted = v.to_vec();
    sorted.sort_by(f64::total_cmp);
    let m = finite(mean(&sorted));
    let var = sample_variance(&sorted).map_err(|e| refused(RangeFault::of(&e)));
    (m, var)
}

/// The sum of the variances in order, or the first one's refusal.
fn sum_of<'a>(
    variances: impl Iterator<Item = &'a Result<f64, AlphaCode>>,
) -> Result<f64, AlphaCode> {
    let mut sum = 0.0;
    for v in variances {
        sum += (*v)?;
    }
    Ok(sum)
}

/// `stats_verify::pearson_clamped`'s r, ±1 within `RHO_ONE_ULPS` units in
/// the last place of it (decision 352's test).
fn correlation(x: &[f64], y: &[f64]) -> Stat {
    match pearson_clamped(x, y) {
        Ok(r) if 1.0 - r.coefficient.abs() <= RHO_ONE_ULPS * f64::EPSILON / 2.0 => {
            Stat::Value(r.coefficient.signum())
        }
        Ok(r) => Stat::Value(r.coefficient),
        Err(e) => Stat::NotComputable(refused(RangeFault::of(&e))),
    }
}

/// A number, or `overflow` when it is not finite.
fn finite(v: f64) -> Stat {
    if v.is_finite() {
        Stat::Value(v)
    } else {
        Stat::NotComputable(AlphaCode::Overflow)
    }
}

/// Every value the same number.
fn constant(v: &[f64]) -> bool {
    v.iter().all(|x| *x == v[0])
}

/// A total's variance within rounding of 0: at most (k · ε · M)², with `k`
/// the items and `magnitude` M the largest row sum of the summands' absolute
/// values. Items that add to a constant (a and 10 − a) leave a variance near
/// 1e-31, not 0, and the formulas would turn that noise into numbers (an
/// alpha of −1e31). A variance the arithmetic refused is not negligible: its
/// refusal is reported where it is used.
fn negligible(variance: &Result<f64, AlphaCode>, k: usize, magnitude: f64) -> bool {
    matches!(variance, Ok(v) if *v <= (k as f64 * f64::EPSILON * magnitude).powi(2))
}

/// The largest of `v`, every value finite and at least 0.
fn largest(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |m, x| m.max(*x))
}

/// A refusal from `stats_verify`, by the range fault it carries.
fn refused(fault: Option<RangeFault>) -> AlphaCode {
    match fault {
        Some(RangeFault::Overflow) => AlphaCode::Overflow,
        Some(RangeFault::Underflow) => AlphaCode::Underflow,
        None => AlphaCode::ArithmeticRefused,
    }
}

/// The record of a run of `spec` on `dataset` that produced `result`. Rows
/// used and the exclusions add up to the rows read. The output hash is
/// SHA-256 of [`AlphaOutcome::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<AlphaParams>,
    dataset: &Dataset,
    result: &AlphaOutcome,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<AlphaCode> {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decision 367: past `stats_dist`'s measured range the interval is not
    /// computable. 100,000 rows reach df2 above 1e7 at 102 items or more.
    #[test]
    fn the_interval_past_the_measured_range_is_not_computable() {
        assert!(matches!(
            feldt(0.8, 100_000, 101),
            Interval::Computed { .. }
        ));
        assert_eq!(
            feldt(0.8, 100_000, 102),
            Interval::NotComputable(AlphaCode::DistributionOutOfRange)
        );
        assert_eq!(
            feldt(0.8, 100_002, 3),
            Interval::NotComputable(AlphaCode::DistributionOutOfRange),
            "df1 past F_QUANTILE_MAX_DF1"
        );
    }

    #[test]
    fn the_reversal_is_on_the_scales_ends() {
        let e = Some(ScaleEnds { min: 1.0, max: 5.0 });
        assert_eq!([1.0, 2.0, 5.0].map(|x| reverse(x, e)), [5.0, 4.0, 1.0]);
        assert_eq!(reverse(2.5, None), 2.5);
    }
}
