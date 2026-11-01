//! **Correlations as an analysis of the user's own data (Research OS,
//! Phase 8H, decisions 336 to 342): `stats.analyze.correlations`.**
//!
//! Not a reproduction of a reported result: that is Level 1's
//! `stats.verify.pearson` and `stats.verify.spearman`. This module computes,
//! between two or more numeric columns, the coefficients the user ticks:
//!
//! * Pearson's r, its t with n − 2 df, the two-tailed p and a 95% interval
//!   by Fisher's z with SE 1/√(n − 3) (R. A. Fisher, 1921; R's `cor.test`);
//! * Spearman's rho (average ranks, then Pearson on the ranks), its t with
//!   n − 2 df, the two-tailed p, and a 95% interval by Fisher's z with
//!   SE √((1 + rho²/2)/(n − 3)) (D. G. Bonett and T. A. Wright, "Sample size
//!   requirements for estimating Pearson, Kendall and Spearman
//!   correlations", Psychometrika 65(1), 23–28, 2000,
//!   <https://doi.org/10.1007/BF02294183>);
//! * Kendall's tau-b, S / √((n0 − n1)(n0 − n2)), its z = S / √var(S) with the
//!   variance corrected for ties (M. G. Kendall, Rank Correlation Methods,
//!   4th ed., Griffin, 1970, ch. 4) and no continuity correction, the
//!   two-tailed normal p, and a 95% interval by Fisher's z with
//!   SE √(0.437/(n − 4)) (E. C. Fieller, H. O. Hartley and E. S. Pearson,
//!   "Tests for rank correlation coefficients. I", Biometrika 44(3/4),
//!   470–481, 1957, <https://doi.org/10.1093/biomet/44.3-4.470>). S is
//!   counted in O(n log n), as W. R. Knight describes in "A computer method
//!   for calculating Kendall's tau with ungrouped data", JASA 61(314),
//!   436–439, 1966, <https://doi.org/10.1080/01621459.1966.10480879>: sort by
//!   x (ties by y), count the pairs tied in x, in y and in both, and the
//!   discordant pairs as the exchanges of a merge sort of y. Written here
//!   from that description; no code was copied.
//!
//! with each variable's n, mean and SD over the rows used. Gaply says which
//! coefficients the user chose, never which they should use.
//!
//! **One formula, one place (decisions 62, 275 and 339).** Pearson's r and t
//! come from `stats_verify::pearson_clamped`, Spearman's from
//! `stats_verify::spearman`; `stats_verify`'s p (statrs's 1 − cdf) is never
//! read: every p of t is `stats_dist::t_two_tailed`, Kendall's is
//! `stats_dist::normal_two_tailed`, every interval's 1.96 is
//! `stats_dist::normal_quantile_975`. Means and variances are
//! `stats_verify::mean` and `sample_variance` on the values ascending.
//! Kendall's tau-b, its variance and the intervals are written once, here.
//! The counts and sums of Kendall's variance are exact integers (u128,
//! i128), each converted to f64 once.
//!
//! **The data (decision 338).** A value is a number that is not a ticked
//! missing code. Listwise, a row is used when every chosen variable has a
//! value; a row left out is counted once, under the first variable in the
//! column order without one (`missing_value` or `missing_code`, that column
//! its parameter). Pairwise, each pair uses the rows in which both have a
//! value, and a row is used when two variables or more have one; a row with
//! fewer is left out as `fewer_than_two_values`, which names no column.
//!
//! **One choice, one spec (decision 338).** Variables in the dataset's
//! column order, coefficients in [`Coefficient`]'s order, missing codes
//! ascending, none repeated; anything else is refused with a closed code,
//! never sorted silently.
//!
//! **Edge cases are decided from the values before each call, never from an
//! error's text.** A pair of fewer than 3 rows (`pair_too_few_rows`); a
//! variable constant over the pair's rows (`variable_constant`); an
//! overflow or underflow `stats_verify` refuses with (`overflow`,
//! `underflow`, by [`RangeFault::of`]); an interval of n ≤ 3, or n ≤ 4 for
//! Kendall (`interval_too_few_rows`). A perfect coefficient is ±1 exactly:
//! Pearson at `stats_verify`'s own boundary, an infinite t, on r clamped to
//! [−1, 1]; Spearman when the two variables rank the pair's rows
//! identically, or exactly in reverse, decided before any call; Kendall when
//! |S| = n0 − n1 = n0 − n2, in integers. A perfect Pearson or Spearman
//! coefficient's t is infinite: the result holds `perfect_coefficient` in
//! its place, never an f64 infinity, and its p is 0. Its interval is
//! [±1, ±1], tanh's limit.
//!
//! **Speed (decision 342).** No cap on variables: a spec whose estimated
//! time, Σ over the chosen coefficients of pairs × rows read × that
//! coefficient's measured seconds per pair-row, passes [`BUDGET_SECONDS`]
//! is refused before the run (`over_budget`).
//!
//! **Pure** (`tests/contracts_are_pure.rs`): no file, network, process,
//! clock or model. The run's check points are the caller's, one before the
//! arithmetic and one before each pair of each chosen coefficient.
//!
//! **Privacy.** [`CorrOutcome`] holds numbers, aliases and closed codes, and
//! no header, label or cell.

use std::cmp::Ordering;
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
use crate::stats_dist::{normal_quantile_975, normal_two_tailed, t_two_tailed};
use crate::stats_l1::binding::suspect_codes_in;
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_ttest::{column, non_finite_cells};
use crate::stats_verify::{
    mean, pearson_clamped, sample_variance, spearman, CorrelationResult, RangeFault,
};

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.correlations";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// Every interval's coverage.
pub const CONFIDENCE_LEVEL: f64 = 0.95;
/// The longest a run may be estimated to take, in seconds (decision 322's
/// figure, decision 342).
pub const BUDGET_SECONDS: f64 = 15.0;

/// Each coefficient's measured worst seconds per pair-row, the one place
/// the budget's rates live (decision 342): the largest over every point of
/// `tests/corr_speed.rs` (release, macOS aarch64, 1 Nov 2026, three runs
/// each, pairwise with every variable missing rows the others have; 100,000
/// rows at k = 10, 20 and 30, 10,000 at 30 and 60, 1,000 at 128):
///
/// * Pearson 1.308e-8 s, at 100,000 rows and k = 10 (0.059 s);
/// * Spearman 1.481e-7 s, at 100,000 rows and k = 30 (6.441 s);
/// * Kendall 1.417e-7 s, at 100,000 rows and k = 30 (6.162 s).
///
/// All three at 100,000 rows and k = 30 took 12.76 to 12.85 s, and the
/// budget estimates 13.18 s, so 30 variables fit at 100,000 rows and 128 at
/// 1,000 (1.67 s measured, 2.46 s estimated).
pub const fn seconds_per_pair_row(c: Coefficient) -> f64 {
    match c {
        Coefficient::Pearson => 1.308e-8,
        Coefficient::Spearman => 1.481e-7,
        Coefficient::Kendall => 1.417e-7,
    }
}

closed! {
    /// Every finding, outcome, exclusion, note and limitation of the
    /// correlations.
    pub enum CorrCode in "stats.corr." {
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
        /// Fewer than two variables are chosen. Param `count`.
        TooFewVariables => "too_few_variables",
        /// A variable is named twice. Param `column`.
        VariableRepeated => "variable_repeated",
        /// The variables are not in the dataset's column order; the first
        /// out of order. Param `column`.
        VariablesOutOfOrder => "variables_out_of_order",
        /// No coefficient is chosen.
        NoCoefficient => "no_coefficient",
        /// A coefficient is named twice. Param `index`.
        CoefficientRepeated => "coefficient_repeated",
        /// The coefficients are not in the order Pearson, Spearman,
        /// Kendall; the first out of order. Param `index`.
        CoefficientsOutOfOrder => "coefficients_out_of_order",
        /// A missing code is NaN or an infinity. Param `index`.
        MissingCodeNotFinite => "missing_code_not_finite",
        /// A missing code is listed twice. Param `index`.
        MissingCodeRepeated => "missing_code_repeated",
        /// The missing codes are not ascending; the first out of order.
        /// Param `index`.
        MissingCodesOutOfOrder => "missing_codes_out_of_order",
        /// A chosen column holds text. Params `column`, `count`.
        VariableHoldsText => "variable_holds_text",
        /// A chosen column holds NaN or an infinity. Params `column`, `count`.
        VariableNonFinite => "variable_non_finite",
        /// A candidate missing code the rule found in a chosen column and the
        /// user did not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Refused: listwise, fewer than 3 rows have a value in every chosen
        /// variable. Param `count`.
        TooFewCompleteRows => "too_few_complete_rows",
        /// Refused: pairwise, no pair has 3 rows or more. Param `count` (the
        /// largest pair's rows).
        NoPairOfThree => "no_pair_of_three",
        /// Refused: the estimated time passes the budget; choose fewer
        /// variables or fewer coefficients. Params `count` (the pairs) and
        /// `found` (the rows read).
        OverBudget => "over_budget",
        /// Not computable: the pair has fewer than 3 rows.
        PairTooFewRows => "pair_too_few_rows",
        /// Not computable: a variable is the same number in every row of the
        /// pair.
        VariableConstant => "variable_constant",
        /// Not computable: a distribution is not measured at this many
        /// degrees of freedom (above 1e7).
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
        /// Not computable: the interval's standard error divides by n − 3
        /// (Pearson, Spearman) or n − 4 (Kendall), and n is that or fewer.
        IntervalTooFewRows => "interval_too_few_rows",
        /// Not computable: the variable has no value in the rows used.
        NoValues => "no_values",
        /// Not computable: the variable has fewer than two values in the
        /// rows used.
        TooFewValues => "too_few_values",
        /// In place of a t: the coefficient is ±1, so t is infinite and its p
        /// is 0.
        PerfectCoefficient => "perfect_coefficient",
        /// Excluded (listwise): the row's first variable without a value has
        /// an empty cell or a missing-value token there. Param `column`.
        MissingValue => "missing_value",
        /// Excluded (listwise): the row's first variable without a value has
        /// a ticked missing code there. Param `column`.
        MissingCode => "missing_code",
        /// Excluded (pairwise): the row has a value in fewer than two chosen
        /// variables.
        FewerThanTwoValues => "fewer_than_two_values",
        /// Limitation: Pearson's p and interval assume the two columns are
        /// roughly normal together; Gaply does not check.
        PearsonAssumesNormality => "pearson_assumes_normality",
        /// Limitation: p values are two-tailed only, and there are no
        /// partial correlations.
        TwoTailedNoPartial => "two_tailed_no_partial",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// A coefficient the user may tick. None is ticked by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coefficient {
    Pearson,
    Spearman,
    Kendall,
}

/// How rows without a value are handled. The user picks one; there is no
/// default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingHandling {
    /// Each pair uses every row in which both of its variables have a value.
    Pairwise,
    /// Every pair uses the rows in which every chosen variable has a value.
    Listwise,
}

/// The correlations' parameters: the variables in the dataset's column
/// order, the coefficients in [`Coefficient`]'s order, the missing handling,
/// and the missing codes ascending, which apply to every chosen variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorrParams {
    pub variables: Vec<Alias>,
    pub coefficients: Vec<Coefficient>,
    pub missing: MissingHandling,
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
    NotComputable(CorrCode),
}

/// A t: a number, or `perfect_coefficient` where it is infinite.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bounded {
    Value(f64),
    Infinite(CorrCode),
}

/// Pearson's and Spearman's t with its df.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TStat {
    pub t: Bounded,
    pub df: f64,
}

/// The test statistic: t for Pearson and Spearman, z for Kendall.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestStatistic {
    T(TStat),
    Z(f64),
}

/// A 95% interval, or why there is none.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Interval {
    Computed { lower: f64, upper: f64 },
    NotComputable(CorrCode),
}

/// One variable's n, mean and SD over the rows used.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptive {
    pub variable: Alias,
    pub n: u64,
    pub mean: Stat,
    pub sd: Stat,
}

/// One pair's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairStats {
    pub estimate: f64,
    pub test: TestStatistic,
    pub p: f64,
    pub interval: Interval,
}

/// A pair's numbers, or why it has none.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PairOutcome {
    Computed(PairStats),
    NotComputable(CorrCode),
}

/// One unordered pair, first before second in the column order, and its
/// rows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pair {
    pub first: Alias,
    pub second: Alias,
    pub n: u64,
    pub outcome: PairOutcome,
}

/// One chosen coefficient's pairs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoefficientTable {
    pub coefficient: Coefficient,
    pub pairs: Vec<Pair>,
}

/// The correlations' result. Every number is finite.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrOutcome {
    /// In the variables' order.
    pub descriptives: Vec<Descriptive>,
    /// The chosen coefficients, in [`Coefficient`]'s order.
    pub coefficients: Vec<CoefficientTable>,
    pub rows_used: u64,
    /// The rows left out, by reason; reasons with no row are not listed.
    pub exclusions: Vec<Exclusion<CorrCode>>,
    pub limitations: Vec<CorrCode>,
}

impl CorrOutcome {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, aliases and closed words serialises")
    }
}

/// What every result says it did not assess.
const LIMITATIONS: [CorrCode; 2] = [
    CorrCode::PearsonAssumesNormality,
    CorrCode::TwoTailedNoPartial,
];

/* ------------------------------------------------------------------------ *
 *  The rows
 * ------------------------------------------------------------------------ */

/// One cell of a chosen variable, as this method reads it.
enum Entry {
    Value(f64),
    Missing,
    Code,
}

/// The chosen variables' entries, column by column, and which rows are
/// used. Never leaves this module, and derives nothing.
struct Rows {
    entries: Vec<Vec<Entry>>,
    used: Vec<bool>,
}

impl Rows {
    fn of(columns: &[&[Cell]], codes: &[f64], missing: MissingHandling) -> Rows {
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
            .map(|r| {
                let values = entries
                    .iter()
                    .filter(|e| matches!(e[r], Entry::Value(_)))
                    .count();
                match missing {
                    MissingHandling::Listwise => values == entries.len(),
                    MissingHandling::Pairwise => values >= 2,
                }
            })
            .collect();
        Rows { entries, used }
    }

    fn value(&self, k: usize, r: usize) -> Option<f64> {
        match self.entries[k][r] {
            Entry::Value(v) => Some(v),
            _ => None,
        }
    }

    fn rows_used(&self) -> u64 {
        self.used.iter().filter(|u| **u).count() as u64
    }

    /// The rows a pair uses: listwise the used rows, pairwise those where
    /// both have a value.
    fn pair(&self, i: usize, j: usize) -> (Vec<f64>, Vec<f64>) {
        let mut x = Vec::new();
        let mut y = Vec::new();
        for r in 0..self.used.len() {
            if !self.used[r] {
                continue;
            }
            if let (Some(a), Some(b)) = (self.value(i, r), self.value(j, r)) {
                x.push(a);
                y.push(b);
            }
        }
        (x, y)
    }

    /// The most rows any pair has.
    fn largest_pair(&self) -> usize {
        let k = self.entries.len();
        let mut best = 0;
        for i in 0..k {
            for j in i + 1..k {
                let n = (0..self.used.len())
                    .filter(|r| self.value(i, *r).is_some() && self.value(j, *r).is_some())
                    .count();
                best = best.max(n);
            }
        }
        best
    }

    /// The rows left out, by reason: listwise each under its first variable
    /// without a value, pairwise as `fewer_than_two_values`.
    fn exclusions(
        &self,
        variables: &[Alias],
        missing: MissingHandling,
    ) -> Vec<Exclusion<CorrCode>> {
        match missing {
            MissingHandling::Pairwise => {
                let count = self.used.iter().filter(|u| !**u).count() as u64;
                if count == 0 {
                    return Vec::new();
                }
                vec![Exclusion {
                    code: CorrCode::FewerThanTwoValues,
                    count,
                    params: BTreeMap::new(),
                }]
            }
            MissingHandling::Listwise => {
                let mut counts = vec![[0u64; 2]; variables.len()];
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
                        (CorrCode::MissingValue, c[0]),
                        (CorrCode::MissingCode, c[1]),
                    ] {
                        if count > 0 {
                            out.push(Exclusion {
                                code,
                                count,
                                params: BTreeMap::from([(
                                    ParamKey::Column,
                                    column_param(variables[k]),
                                )]),
                            });
                        }
                    }
                }
                out
            }
        }
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
    parameters: CorrParams,
) -> OperationSpec<CorrParams> {
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

fn column_param(alias: Alias) -> ParamValue<CorrCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

/// The number of unordered pairs of `k` variables.
fn pairs_of(k: usize) -> usize {
    k * k.saturating_sub(1) / 2
}

/// The estimated seconds of a run of `params` on `rows` rows read: Σ over
/// the chosen coefficients of pairs × rows × that coefficient's
/// [`seconds_per_pair_row`] (decision 342).
pub fn estimated_seconds(params: &CorrParams, rows: usize) -> f64 {
    let pair_rows = (pairs_of(params.variables.len()) as f64) * rows as f64;
    params
        .coefficients
        .iter()
        .map(|c| pair_rows * seconds_per_pair_row(*c))
        .sum()
}

/// Shared by `validate` and `run`: the errors in a fixed order (missing
/// codes, variables, coefficients, the budget, the columns, then the rows),
/// and the chosen columns' cells when every parameter is good.
fn check<'d>(
    dataset: &'d Dataset,
    params: &CorrParams,
) -> (Vec<Diagnostic<CorrCode>>, Option<Vec<&'d [Cell]>>) {
    let mut errors = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(
                Diagnostic::error(CorrCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64),
            );
        }
    }
    for (i, w) in params.missing_codes.windows(2).enumerate() {
        if w[0] == w[1] {
            errors.push(
                Diagnostic::error(CorrCode::MissingCodeRepeated).int(ParamKey::Index, i as i64 + 1),
            );
        } else if w[0] > w[1] {
            errors.push(
                Diagnostic::error(CorrCode::MissingCodesOutOfOrder)
                    .int(ParamKey::Index, i as i64 + 1),
            );
        }
    }
    let vars = &params.variables;
    if vars.len() < 2 {
        errors.push(
            Diagnostic::error(CorrCode::TooFewVariables).int(ParamKey::Count, vars.len() as i64),
        );
    }
    for w in vars.windows(2) {
        if w[0] == w[1] {
            errors.push(
                Diagnostic::error(CorrCode::VariableRepeated)
                    .with(ParamKey::Column, column_param(w[1])),
            );
        } else if w[0] > w[1] {
            errors.push(
                Diagnostic::error(CorrCode::VariablesOutOfOrder)
                    .with(ParamKey::Column, column_param(w[1])),
            );
        }
    }
    let coefs = &params.coefficients;
    if coefs.is_empty() {
        errors.push(Diagnostic::error(CorrCode::NoCoefficient));
    }
    for (i, w) in coefs.windows(2).enumerate() {
        if w[0] == w[1] {
            errors.push(
                Diagnostic::error(CorrCode::CoefficientRepeated).int(ParamKey::Index, i as i64 + 1),
            );
        } else if w[0] > w[1] {
            errors.push(
                Diagnostic::error(CorrCode::CoefficientsOutOfOrder)
                    .int(ParamKey::Index, i as i64 + 1),
            );
        }
    }
    let rows = dataset.rows();
    if errors.is_empty() && estimated_seconds(params, rows) > BUDGET_SECONDS {
        errors.push(
            Diagnostic::error(CorrCode::OverBudget)
                .int(ParamKey::Count, pairs_of(vars.len()) as i64)
                .int(ParamKey::Found, rows as i64),
        );
    }
    let mut cells = Vec::with_capacity(vars.len());
    for alias in vars {
        match column(dataset, *alias) {
            None => errors.push(
                Diagnostic::error(CorrCode::UnknownColumn)
                    .with(ParamKey::Column, column_param(*alias)),
            ),
            Some((summary, c)) => {
                let non_finite = non_finite_cells(summary);
                let texts = c.iter().filter(|x| matches!(x, Cell::Text(_))).count() as i64;
                if non_finite > 0 {
                    errors.push(
                        Diagnostic::error(CorrCode::VariableNonFinite)
                            .with(ParamKey::Column, column_param(*alias))
                            .int(ParamKey::Count, non_finite as i64),
                    );
                } else if texts > 0 {
                    errors.push(
                        Diagnostic::error(CorrCode::VariableHoldsText)
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
    let r = Rows::of(&cells, &params.missing_codes, params.missing);
    match params.missing {
        MissingHandling::Listwise => {
            let complete = r.rows_used();
            if complete < 3 {
                errors.push(
                    Diagnostic::error(CorrCode::TooFewCompleteRows)
                        .int(ParamKey::Count, complete as i64),
                );
            }
        }
        MissingHandling::Pairwise => {
            let best = r.largest_pair();
            if best < 3 {
                errors.push(
                    Diagnostic::error(CorrCode::NoPairOfThree).int(ParamKey::Count, best as i64),
                );
            }
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
/// error; a warning for a candidate missing code in a chosen column that the
/// user did not list.
pub fn validate(dataset: &Dataset, spec: &OperationSpec<CorrParams>) -> ValidationReport<CorrCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID
    {
        unsupported.push(Diagnostic::error(CorrCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(CorrCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(CorrCode::InputMismatch));
            }
        }
        inputs => errors.push(
            Diagnostic::error(CorrCode::InputCount).int(ParamKey::Count, inputs.len() as i64),
        ),
    }
    let params = &spec.parameters;
    errors.extend(check(dataset, params).0);
    let mut warnings = Vec::new();
    for alias in &params.variables {
        if let Some((_, cells)) = column(dataset, *alias) {
            for s in suspect_codes_in(alias.0, cells) {
                if !params.missing_codes.contains(&f64::from(s.code)) {
                    warnings.push(
                        Diagnostic::warning(CorrCode::SuspectMissingCode)
                            .with(ParamKey::Column, column_param(*alias))
                            .int(ParamKey::Value, i64::from(s.code))
                            .int(ParamKey::Count, s.cells as i64),
                    );
                }
            }
        }
    }
    let rows = dataset.rows() as u64;
    let columns = params.variables.len() as u64;
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
    Refused(Diagnostic<CorrCode>),
    /// The caller's check point stopped the run.
    Stopped { reason: B },
}

/// How many check points a run of `params` calls: one before the arithmetic,
/// and one before each pair of each chosen coefficient (decision 342). Only
/// the spec is needed.
pub fn check_points(params: &CorrParams) -> usize {
    1 + params.coefficients.len() * pairs_of(params.variables.len())
}

/// [`run`] with a check point that never stops.
pub fn compute(
    dataset: &Dataset,
    params: &CorrParams,
) -> Result<CorrOutcome, Diagnostic<CorrCode>> {
    match run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The correlations. `checkpoint(0)` is called after the parameters are
/// checked and before the arithmetic, and `checkpoint(i)` before the i-th
/// pair, counted across the chosen coefficients from 1; a `Break` stops the
/// run there. What a stop means is the caller's.
pub fn run<B>(
    dataset: &Dataset,
    params: &CorrParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<CorrOutcome, Halt<B>> {
    let (errors, cells) = check(dataset, params);
    if let Some(e) = errors.into_iter().next() {
        return Err(Halt::Refused(e));
    }
    let cells = cells.expect("no error means the columns are good");
    if let ControlFlow::Break(reason) = checkpoint(0) {
        return Err(Halt::Stopped { reason });
    }
    let rows = Rows::of(&cells, &params.missing_codes, params.missing);
    let k = params.variables.len();
    let descriptives = (0..k)
        .map(|v| {
            let mut values: Vec<f64> = (0..rows.used.len())
                .filter(|r| rows.used[*r])
                .filter_map(|r| rows.value(v, r))
                .collect();
            values.sort_by(f64::total_cmp);
            descriptive(params.variables[v], &values)
        })
        .collect();
    // Listwise, every pair has the same rows: read once.
    let listwise: Option<Vec<Vec<f64>>> =
        (params.missing == MissingHandling::Listwise).then(|| {
            (0..k)
                .map(|v| {
                    (0..rows.used.len())
                        .filter(|r| rows.used[*r])
                        .filter_map(|r| rows.value(v, r))
                        .collect()
                })
                .collect()
        });
    let mut coefficients = Vec::with_capacity(params.coefficients.len());
    let mut point = 0usize;
    for c in &params.coefficients {
        let mut pairs = Vec::with_capacity(pairs_of(k));
        for i in 0..k {
            for j in i + 1..k {
                point += 1;
                if let ControlFlow::Break(reason) = checkpoint(point) {
                    return Err(Halt::Stopped { reason });
                }
                let owned;
                let (x, y): (&[f64], &[f64]) = match &listwise {
                    Some(cols) => (&cols[i], &cols[j]),
                    None => {
                        owned = rows.pair(i, j);
                        (&owned.0, &owned.1)
                    }
                };
                pairs.push(Pair {
                    first: params.variables[i],
                    second: params.variables[j],
                    n: x.len() as u64,
                    outcome: pair(*c, x, y),
                });
            }
        }
        coefficients.push(CoefficientTable {
            coefficient: *c,
            pairs,
        });
    }
    Ok(CorrOutcome {
        descriptives,
        coefficients,
        rows_used: rows.rows_used(),
        exclusions: rows.exclusions(&params.variables, params.missing),
        limitations: LIMITATIONS.to_vec(),
    })
}

/// A refusal from `stats_verify`, by the range fault it carries.
fn refused(fault: Option<RangeFault>) -> CorrCode {
    match fault {
        Some(RangeFault::Overflow) => CorrCode::Overflow,
        Some(RangeFault::Underflow) => CorrCode::Underflow,
        None => CorrCode::ArithmeticRefused,
    }
}

/// A variable's n, mean and SD over `values`, ascending.
fn descriptive(variable: Alias, values: &[f64]) -> Descriptive {
    let n = values.len();
    let m = if n == 0 {
        Stat::NotComputable(CorrCode::NoValues)
    } else {
        match mean(values) {
            v if v.is_finite() => Stat::Value(v),
            _ => Stat::NotComputable(CorrCode::Overflow),
        }
    };
    let sd = if n < 2 {
        Stat::NotComputable(CorrCode::TooFewValues)
    } else {
        match sample_variance(values) {
            Ok(v) if v.sqrt().is_finite() => Stat::Value(v.sqrt()),
            Ok(_) => Stat::NotComputable(CorrCode::Overflow),
            Err(e) => Stat::NotComputable(refused(RangeFault::of(&e))),
        }
    };
    Descriptive {
        variable,
        n: n as u64,
        mean: m,
        sd,
    }
}

fn constant(v: &[f64]) -> bool {
    v.iter().all(|x| *x == v[0])
}

/// The two numbers compared as numbers: -0 equals 0. Every value is finite.
fn order(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b).expect("finite values compare")
}

/// One coefficient of one pair (decision 338's edge cases first).
fn pair(c: Coefficient, x: &[f64], y: &[f64]) -> PairOutcome {
    let n = x.len();
    if n < 3 {
        return PairOutcome::NotComputable(CorrCode::PairTooFewRows);
    }
    if constant(x) || constant(y) {
        return PairOutcome::NotComputable(CorrCode::VariableConstant);
    }
    let computed = match c {
        Coefficient::Pearson => pearson_clamped(x, y)
            .map_err(|e| refused(RangeFault::of(&e)))
            .and_then(|r| t_stats(&r, n, c)),
        Coefficient::Spearman => match rank_sign(x, y) {
            Some(sign) => Ok(PairStats {
                estimate: sign,
                test: TestStatistic::T(TStat {
                    t: Bounded::Infinite(CorrCode::PerfectCoefficient),
                    df: (n - 2) as f64,
                }),
                p: 0.0,
                interval: interval(c, sign, n),
            }),
            None => spearman(x, y)
                .map_err(|e| refused(RangeFault::of(&e)))
                .and_then(|r| t_stats(&r, n, c)),
        },
        Coefficient::Kendall => kendall(x, y),
    };
    match computed {
        Ok(s) => PairOutcome::Computed(s),
        Err(code) => PairOutcome::NotComputable(code),
    }
}

/// Pearson's or Spearman's numbers from `stats_verify`'s coefficient and t;
/// the p from `stats_dist::t_two_tailed`, never `stats_verify`'s.
fn t_stats(r: &CorrelationResult, n: usize, c: Coefficient) -> Result<PairStats, CorrCode> {
    let df = (n - 2) as f64;
    let (t, p) = if r.statistic.is_infinite() {
        (Bounded::Infinite(CorrCode::PerfectCoefficient), 0.0)
    } else {
        let p = t_two_tailed(r.statistic, df).ok_or(CorrCode::DistributionOutOfRange)?;
        (Bounded::Value(r.statistic), p)
    };
    Ok(PairStats {
        estimate: r.coefficient,
        test: TestStatistic::T(TStat { t, df }),
        p,
        interval: interval(c, r.coefficient, n),
    })
}

/// Spearman's perfect predicate, from the values before any call: +1 when
/// the two variables put the rows in the same order with the same ties, −1
/// when in exactly the reverse order, otherwise none. Sorted by x (ties by
/// y), consecutive rows must tie in x exactly when they tie in y, and y must
/// never fall (or, for −1, never rise).
fn rank_sign(x: &[f64], y: &[f64]) -> Option<f64> {
    let mut idx: Vec<usize> = (0..x.len()).collect();
    idx.sort_by(|&a, &b| order(x[a], x[b]).then(order(y[a], y[b])));
    let ties_agree = idx
        .windows(2)
        .all(|w| (x[w[0]] == x[w[1]]) == (y[w[0]] == y[w[1]]));
    if !ties_agree {
        return None;
    }
    if idx.windows(2).all(|w| y[w[0]] <= y[w[1]]) {
        Some(1.0)
    } else if idx.windows(2).all(|w| y[w[0]] >= y[w[1]]) {
        Some(-1.0)
    } else {
        None
    }
}

/// A 95% interval by Fisher's z with the coefficient's SE (the module doc);
/// [±1, ±1] at a perfect coefficient.
fn interval(c: Coefficient, estimate: f64, n: usize) -> Interval {
    let fewest = match c {
        Coefficient::Kendall => 5,
        Coefficient::Pearson | Coefficient::Spearman => 4,
    };
    if n < fewest {
        return Interval::NotComputable(CorrCode::IntervalTooFewRows);
    }
    if estimate.abs() == 1.0 {
        return Interval::Computed {
            lower: estimate,
            upper: estimate,
        };
    }
    let n = n as f64;
    let se = match c {
        Coefficient::Pearson => 1.0 / (n - 3.0).sqrt(),
        Coefficient::Spearman => ((1.0 + estimate * estimate / 2.0) / (n - 3.0)).sqrt(),
        Coefficient::Kendall => (0.437 / (n - 4.0)).sqrt(),
    };
    let z = estimate.atanh();
    let half = normal_quantile_975() * se;
    Interval::Computed {
        lower: (z - half).tanh(),
        upper: (z + half).tanh(),
    }
}

/// The sizes of the runs of equal values in `v`, which is sorted.
fn runs(v: &[f64]) -> Vec<u128> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let mut j = i + 1;
        while j < v.len() && v[j] == v[i] {
            j += 1;
        }
        out.push((j - i) as u128);
        i = j;
    }
    out
}

/// Sorts `v` ascending by a merge sort and returns the pairs it exchanged:
/// those with the earlier strictly above the later (Knight's discordant
/// count; equal values are not exchanged).
fn exchanges(v: &mut [f64]) -> u128 {
    let n = v.len();
    let mut buf = v.to_vec();
    let mut count = 0u128;
    let mut width = 1;
    while width < n {
        let mut lo = 0;
        while lo < n {
            let mid = (lo + width).min(n);
            let hi = (lo + 2 * width).min(n);
            let (mut i, mut j, mut k) = (lo, mid, lo);
            while i < mid && j < hi {
                if v[j] < v[i] {
                    buf[k] = v[j];
                    count += (mid - i) as u128;
                    j += 1;
                } else {
                    buf[k] = v[i];
                    i += 1;
                }
                k += 1;
            }
            buf[k..k + (mid - i)].copy_from_slice(&v[i..mid]);
            k += mid - i;
            buf[k..k + (hi - j)].copy_from_slice(&v[j..hi]);
            lo = hi;
        }
        v.copy_from_slice(&buf);
        width *= 2;
    }
    count
}

/// Kendall's tau-b, z and p (the module doc), in exact integers until each
/// sum is converted to f64 once. Neither variable is constant.
fn kendall(x: &[f64], y: &[f64]) -> Result<PairStats, CorrCode> {
    let n = x.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| order(x[a], x[b]).then(order(y[a], y[b])));
    let xs: Vec<f64> = idx.iter().map(|&i| x[i]).collect();
    let mut ys: Vec<f64> = idx.iter().map(|&i| y[i]).collect();
    // Pairs tied in both: runs of equal (x, y) in that order.
    let mut n3 = 0u128;
    let mut i = 0;
    while i < n {
        let mut j = i + 1;
        while j < n && xs[j] == xs[i] && ys[j] == ys[i] {
            j += 1;
        }
        let t = (j - i) as u128;
        n3 += t * (t - 1) / 2;
        i = j;
    }
    let tx = runs(&xs);
    let discordant = exchanges(&mut ys);
    let ty = runs(&ys);
    let nn = n as u128;
    let n0 = nn * (nn - 1) / 2;
    let pairs = |t: &[u128]| t.iter().map(|t| t * (t - 1) / 2).sum::<u128>();
    let (n1, n2) = (pairs(&tx), pairs(&ty));
    let s: i128 = (n0 + n3) as i128 - (n1 + n2) as i128 - 2 * discordant as i128;
    let (dx, dy) = (n0 - n1, n0 - n2);
    if dx == 0 || dy == 0 {
        return Err(CorrCode::ArithmeticRefused);
    }
    let tau = if s.unsigned_abs() == dx && dx == dy {
        s.signum() as f64
    } else {
        s as f64 / ((dx * dy) as f64).sqrt()
    };
    let v = |t: &[u128]| t.iter().map(|t| t * (t - 1) * (2 * t + 5)).sum::<u128>();
    let s1 = |t: &[u128]| t.iter().map(|t| t * (t - 1)).sum::<u128>();
    // A run of one or two adds 0, and t - 2 is not defined below 2 in u128.
    let s2 = |t: &[u128]| {
        t.iter()
            .filter(|t| **t >= 3)
            .map(|t| t * (t - 1) * (t - 2))
            .sum::<u128>()
    };
    let v0 = nn * (nn - 1) * (2 * nn + 5);
    let first = v0 as i128 - v(&tx) as i128 - v(&ty) as i128;
    let v1 = s1(&tx) * s1(&ty);
    let v2 = s2(&tx) * s2(&ty);
    let var = first as f64 / 18.0
        + v1 as f64 / (2 * nn * (nn - 1)) as f64
        + v2 as f64 / (9 * nn * (nn - 1) * (nn - 2)) as f64;
    if !(var > 0.0 && var.is_finite()) {
        return Err(CorrCode::ArithmeticRefused);
    }
    let z = s as f64 / var.sqrt();
    Ok(PairStats {
        estimate: tau,
        test: TestStatistic::Z(z),
        p: normal_two_tailed(z),
        interval: interval(Coefficient::Kendall, tau, n),
    })
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows
/// used and the exclusions add up to the rows read. The output hash is
/// SHA-256 of [`CorrOutcome::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<CorrParams>,
    dataset: &Dataset,
    result: &CorrOutcome,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<CorrCode> {
    let parameters = serde_json::to_vec(&spec.parameters)
        .expect("parameters of aliases, words and numbers serialise");
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
