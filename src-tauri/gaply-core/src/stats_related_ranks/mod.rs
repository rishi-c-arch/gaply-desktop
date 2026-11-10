//! **The Wilcoxon signed-rank and Friedman tests as an analysis of the user's
//! own data (Research OS, Phase 8Z-1, decisions 636 to 639):
//! `stats.analyze.related_ranks`.**
//!
//! One method over two or more numeric columns measured on the same rows,
//! compared by the ranks of their values. The test is chosen by the count of
//! columns, as SPSS's related-samples nonparametric tests choose:
//!
//! * two, the Wilcoxon signed-rank test, F. Wilcoxon, "Individual comparisons
//!   by ranking methods", Biometrics Bulletin 1(6), 80–83, 1945,
//!   <https://doi.org/10.2307/3001968>: d = first − second for each complete
//!   row, as the paired t-test's; a d of zero dropped, as R's
//!   `wilcox.test(paired = TRUE, exact = FALSE)` drops it; the ranks of |d|
//!   through `stats_verify::average_ranks`, V the sum of the positive d's
//!   ranks, R's V, and T− = n (n + 1) / 2 − V; z = (V − n (n + 1) / 4) / σ with
//!   σ² = n (n + 1) (2n + 1) / 24 − Σ(t³ − t) / 48, with no continuity
//!   correction; its two-sided p, `stats_dist::normal_two_tailed`; the exact
//!   two-sided p, min(1, 2 min(P(V ≤ v), P(V ≥ v))), from
//!   `stats_dist::signed_rank_exact_tails`, only when no |d| is tied, no d is
//!   zero and fewer than 50 pairs remain; the matched-pairs rank-biserial
//!   correlation, (V − T−) / (n (n + 1) / 2), positive when the first column's
//!   values tend to be the larger (D. S. Kerby, "The simple difference
//!   formula: an approach to teaching nonparametric correlation", Comprehensive
//!   Psychology 3, 2014, <https://doi.org/10.2466/11.IT.3.1>);
//! * three or more, the Friedman test, M. Friedman, "The use of ranks to avoid
//!   the assumption of normality implicit in the analysis of variance", JASA
//!   32(200), 675–701, 1937, <https://doi.org/10.1080/01621459.1937.10503522>:
//!   within each complete row the k values ranked from 1, R_j each column's
//!   rank sum, Q = (12 / (n k (k + 1))) Σ D_j² / C with
//!   D_j = R_j − n (k + 1) / 2, exact since both are half-integers, and
//!   C = 1 − Σ over rows Σ(t³ − t) / (n (k³ − k)), computed as the exact integer
//!   n (k³ − k) − Σ(t³ − t) over n (k³ − k), so it does not cancel when nearly
//!   every row is tied; df = k − 1; its p,
//!   `stats_dist::chi_square_upper`; Kendall's W, Q / (n (k − 1)).
//!
//! **One formula, one place (decision 636).** Ranks come from
//! `stats_verify::average_ranks`, tie runs from `stats_corr::runs`, medians
//! from `stats_anova::median`, every p from `stats_dist`.
//!
//! **The data.** A row is used only when every chosen column has a number that
//! is not a ticked code. A row left out is counted once, under the first column
//! in the list's order without one, as `missing_value` or `missing_code`, as
//! Cronbach's alpha's are. Any column may be a scale score.
//!
//! **Edge cases are decided from the values, never from an error's text.** Every
//! d zero leaves n = 0: z, its p and the rank-biserial correlation are not
//! computable (`all_differences_zero`). Every row's values tied leaves C at 0:
//! Q, its p and W are not computable (`all_values_tied`). A difference that
//! overflows from two finite values is ranked by its size, an infinity above
//! every finite difference, as R ranks it; any statistic it makes non-finite is
//! not computable with `overflow`, statistic by statistic, never a refusal of
//! the run.
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
use crate::stats_anova::median;
use crate::stats_corr::runs;
use crate::stats_desc::{
    input_ref, preparation_holds, score_code_count, suspect_codes_unless_score, Alias,
};
use crate::stats_dist::{chi_square_upper, normal_two_tailed, signed_rank_exact_tails};
use crate::stats_l1::dataset::{Cell, Dataset};
use crate::stats_ttest::{column, non_finite_cells};
use crate::stats_verify::average_ranks;

pub mod export;

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.analyze.related_ranks";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// The most columns a run takes.
pub const MAX_RELATED_COLUMNS: usize = 50;
/// The count of pairs from which the exact p is no longer given: there must be
/// fewer than this (decision 636).
pub const EXACT_BELOW: u64 = 50;
/// Fewer non-zero differences, or complete rows, than this is noted.
pub const ROWS_NOTE_BELOW: u64 = 5;

closed! {
    /// Every refusal, outcome, exclusion, note and limitation of the related
    /// rank tests.
    pub enum RelatedRanksCode in "stats.related_ranks." {
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
        /// Fewer than two columns. Param `count`.
        TooFewColumns => "too_few_columns",
        /// More columns than a run takes. Params `count`, `limit`.
        TooManyColumns => "too_many_columns",
        /// A column is named twice. Param `column`.
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
        /// A candidate missing code the rule found in a column and the user
        /// did not list. Params `column`, `value`, `count`.
        SuspectMissingCode => "suspect_missing_code",
        /// Not computable: every difference is zero, so no pair remains.
        AllDifferencesZero => "all_differences_zero",
        /// Not computable: every row's values are tied, so the tie correction
        /// of Q is 0.
        AllValuesTied => "all_values_tied",
        /// Not computable: the chi-square is not measured at this many degrees
        /// of freedom.
        DistributionOutOfRange => "distribution_out_of_range",
        /// Not computable: a sum, product, quotient or root a double cannot
        /// hold.
        Overflow => "overflow",
        /// Note: the exact p is not given, two differences being tied in size.
        ExactNotGivenTies => "exact_not_given_ties",
        /// Note: the exact p is not given, a difference being zero.
        ExactNotGivenZeros => "exact_not_given_zeros",
        /// Note: the exact p is not given, there being 50 pairs or more.
        ExactNotGivenSize => "exact_not_given_size",
        /// Note: a pair whose two values are equal was left out of the test.
        ZeroDifferencesDropped => "zero_differences_dropped",
        /// Note: fewer than 5 non-zero differences, or complete rows, where
        /// the approximation is rougher.
        RowsBelowFive => "rows_below_five",
        /// Excluded: the column's cell is empty or a missing-value token.
        MissingValue => "missing_value",
        /// Excluded: the column's cell is a value the user listed.
        MissingCode => "missing_code",
        /// Limitation: no pairwise comparisons after the Friedman test.
        NoPairwiseComparisons => "no_pairwise_comparisons",
        /// Limitation: each Wilcoxon p is two-sided.
        TwoSidedOnly => "two_sided_only",
        /// Limitation: no Hodges-Lehmann estimate of the differences, and no
        /// interval for it.
        NoLocationEstimate => "no_location_estimate",
        /// Limitation: the exact p only without tied differences or zeros and
        /// below 50 pairs.
        ExactWithoutTiesOrZerosOnly => "exact_without_ties_or_zeros_only",
        /// Limitation: a zero pair is dropped; Pratt's method is not offered.
        PrattNotOffered => "pratt_not_offered",
        /// Limitation: no sign test, marginal homogeneity test or Cochran's Q.
        NoFurtherRelatedTests => "no_further_related_tests",
        /// Limitation: each row counts once; no weights.
        NoWeights => "no_weights",
        /// Limitation: the differences and the signs follow the order the user
        /// chose the columns in.
        ColumnOrder => "column_order",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// The related rank tests' parameters. `columns` are in the user's order, which
/// fixes the differences, first minus second. `missing_codes` apply by value to
/// every chosen column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelatedRanksParams {
    pub columns: Vec<Alias>,
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
    NotComputable(RelatedRanksCode),
}

/// One chosen column, in the list's order, and its median over the complete
/// rows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedColumn {
    pub column: Alias,
    pub median: f64,
}

/// The ranks of one sign's differences.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignRanks {
    pub count: u64,
    pub rank_sum: f64,
    /// Absent when no difference has this sign.
    pub mean_rank: Option<f64>,
}

/// The Wilcoxon signed-rank test of two columns.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Wilcoxon {
    pub first: Alias,
    pub second: Alias,
    /// The pairs with a non-zero difference.
    pub n: u64,
    pub zero_differences: u64,
    /// The runs of two or more equal |d|.
    pub tie_runs: u64,
    pub negative: SignRanks,
    pub positive: SignRanks,
    /// The sum of the positive differences' ranks: R's V.
    pub v: f64,
    /// n (n + 1) / 2 − V.
    pub t_minus: f64,
    pub z: Stat,
    /// The normal approximation's two-sided p, without a continuity
    /// correction.
    pub p: Stat,
    /// The exact two-sided p, given only without tied differences or zeros and
    /// below 50 pairs; the notes say why it is absent.
    pub exact_p: Option<f64>,
    /// (V − T−) / (n (n + 1) / 2).
    pub rank_biserial: Stat,
    pub median_difference: Stat,
}

/// One column's ranks in the Friedman test.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriedmanColumn {
    pub column: Alias,
    pub rank_sum: f64,
    pub mean_rank: f64,
}

/// The Friedman test of three or more columns.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Friedman {
    pub columns: Vec<FriedmanColumn>,
    pub q: Stat,
    /// k − 1.
    pub df: f64,
    pub p: Stat,
    /// Q / (n (k − 1)).
    pub kendalls_w: Stat,
}

/// The test the count of columns chose.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelatedTest {
    Wilcoxon(Wilcoxon),
    Friedman(Friedman),
}

/// The related rank tests' result. Every number is finite.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedRanksOutcome {
    /// In the list's order.
    pub columns: Vec<RelatedColumn>,
    /// The complete rows, zero differences among them.
    pub rows_used: u64,
    pub test: RelatedTest,
    /// The rows left out, by column and reason; none with no row is listed.
    pub exclusions: Vec<Exclusion<RelatedRanksCode>>,
    /// In the order the codes are declared, when they apply.
    pub notes: Vec<RelatedRanksCode>,
    pub limitations: Vec<RelatedRanksCode>,
}

impl RelatedRanksOutcome {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, closed words and aliases serialises")
    }
}

/// What every result says it does not give.
const LIMITATIONS: [RelatedRanksCode; 8] = [
    RelatedRanksCode::NoPairwiseComparisons,
    RelatedRanksCode::TwoSidedOnly,
    RelatedRanksCode::NoLocationEstimate,
    RelatedRanksCode::ExactWithoutTiesOrZerosOnly,
    RelatedRanksCode::PrattNotOffered,
    RelatedRanksCode::NoFurtherRelatedTests,
    RelatedRanksCode::NoWeights,
    RelatedRanksCode::ColumnOrder,
];

/* ------------------------------------------------------------------------ *
 *  The one reading of the rows
 * ------------------------------------------------------------------------ */

/// The complete rows' values, one vector per column in the list's order, and
/// each column's rows left out. Never leaves this module, and derives nothing.
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
    parameters: RelatedRanksParams,
) -> OperationSpec<RelatedRanksParams> {
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

fn column_param(alias: Alias) -> ParamValue<RelatedRanksCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

struct Checked<'d> {
    errors: Vec<Diagnostic<RelatedRanksCode>>,
    ready: Option<Vec<&'d [Cell]>>,
}

/// Shared by `validate` and `run`: the errors in a fixed order (the missing
/// codes, the list, each column, a code equal to a score's value), then, only
/// when there is no other, no complete row.
fn check<'d>(dataset: &'d Dataset, params: &RelatedRanksParams) -> Checked<'d> {
    let mut errors = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(
                Diagnostic::error(RelatedRanksCode::MissingCodeNotFinite)
                    .int(ParamKey::Index, i as i64),
            );
        }
    }
    let count = params.columns.len();
    if count < 2 {
        errors.push(
            Diagnostic::error(RelatedRanksCode::TooFewColumns).int(ParamKey::Count, count as i64),
        );
    } else if count > MAX_RELATED_COLUMNS {
        errors.push(
            Diagnostic::error(RelatedRanksCode::TooManyColumns)
                .int(ParamKey::Count, count as i64)
                .int(ParamKey::Limit, MAX_RELATED_COLUMNS as i64),
        );
    }
    let mut seen = BTreeSet::new();
    for alias in &params.columns {
        if !seen.insert(*alias) {
            errors.push(
                Diagnostic::error(RelatedRanksCode::ColumnRepeated)
                    .with(ParamKey::Column, column_param(*alias)),
            );
        }
    }
    let mut cells = Vec::with_capacity(count);
    for alias in &params.columns {
        let Some((summary, column_cells)) = column(dataset, *alias) else {
            errors.push(
                Diagnostic::error(RelatedRanksCode::UnknownColumn)
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
                Diagnostic::error(RelatedRanksCode::ColumnNonFinite)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, non_finite as i64),
            );
        } else if texts > 0 {
            errors.push(
                Diagnostic::error(RelatedRanksCode::ColumnHoldsText)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, texts),
            );
        }
        cells.push(column_cells);
    }
    let mut scanned = BTreeSet::new();
    for alias in params.columns.iter().filter(|a| scanned.insert(**a)) {
        let collisions = score_code_count(dataset, *alias, &params.missing_codes);
        if collisions > 0 {
            errors.push(
                Diagnostic::error(RelatedRanksCode::CodeEqualsScore)
                    .with(ParamKey::Column, column_param(*alias))
                    .int(ParamKey::Count, collisions as i64),
            );
        }
    }
    if errors.is_empty() {
        if Rows::of(&cells, &params.missing_codes).complete() == 0 {
            errors.push(Diagnostic::error(RelatedRanksCode::NoCompleteRows));
        }
        return Checked {
            errors,
            ready: Some(cells),
        };
    }
    Checked {
        errors,
        ready: None,
    }
}

/// Checks the spec against the dataset. Unsupported when it names another
/// capability, version or workspace; invalid on a wrong input or a parameter
/// error; a warning for each candidate missing code in a column that the user
/// did not list. Nothing needs input.
pub fn validate(
    dataset: &Dataset,
    spec: &OperationSpec<RelatedRanksParams>,
) -> ValidationReport<RelatedRanksCode> {
    let mut unsupported = Vec::new();
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID
    {
        unsupported.push(Diagnostic::error(RelatedRanksCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(RelatedRanksCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }
    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(RelatedRanksCode::InputMismatch));
            }
        }
        inputs => errors.push(
            Diagnostic::error(RelatedRanksCode::InputCount)
                .int(ParamKey::Count, inputs.len() as i64),
        ),
    }
    if !preparation_holds(dataset, spec.preparation.as_ref()) {
        errors.push(Diagnostic::error(RelatedRanksCode::PreparationMismatch));
    }
    let params = &spec.parameters;
    errors.extend(check(dataset, params).errors);
    let mut warnings = Vec::new();
    let mut warned = BTreeSet::new();
    for alias in &params.columns {
        if !warned.insert(*alias) {
            continue;
        }
        if let Some((_, cells)) = column(dataset, *alias) {
            for s in suspect_codes_unless_score(dataset, *alias, cells) {
                if !params.missing_codes.contains(&f64::from(s.code)) {
                    warnings.push(
                        Diagnostic::warning(RelatedRanksCode::SuspectMissingCode)
                            .with(ParamKey::Column, column_param(*alias))
                            .int(ParamKey::Value, i64::from(s.code))
                            .int(ParamKey::Count, s.cells as i64),
                    );
                }
            }
        }
    }
    let rows = dataset.rows() as u64;
    let columns = params.columns.len() as u64;
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
    Refused(Diagnostic<RelatedRanksCode>),
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
    params: &RelatedRanksParams,
) -> Result<RelatedRanksOutcome, Diagnostic<RelatedRanksCode>> {
    match run(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason }) => match reason {},
    }
}

/// The related rank tests. `checkpoint(0)` is called after the parameters are
/// checked and before the arithmetic; a `Break` stops the run there. What a stop
/// means is the caller's.
pub fn run<B>(
    dataset: &Dataset,
    params: &RelatedRanksParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<RelatedRanksOutcome, Halt<B>> {
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
    let columns: Vec<RelatedColumn> = params
        .columns
        .iter()
        .zip(&rows.values)
        .map(|(alias, values)| {
            let mut sorted = values.clone();
            sorted.sort_by(f64::total_cmp);
            RelatedColumn {
                column: *alias,
                median: median(&sorted),
            }
        })
        .collect();
    let mut notes = Vec::new();
    let test = if params.columns.len() == 2 {
        let (t, ws_notes) = wilcoxon(params.columns[0], params.columns[1], &rows.values[0], &rows.values[1])
            .map_err(refuse)?;
        notes.extend(ws_notes);
        RelatedTest::Wilcoxon(t)
    } else {
        let t = friedman(&params.columns, &rows.values);
        if (rows.complete() as u64) < ROWS_NOTE_BELOW {
            notes.push(RelatedRanksCode::RowsBelowFive);
        }
        RelatedTest::Friedman(t)
    };
    let mut exclusions = Vec::new();
    for (alias, counts) in params.columns.iter().zip(&rows.excluded) {
        for (code, count) in [
            (RelatedRanksCode::MissingValue, counts[0]),
            (RelatedRanksCode::MissingCode, counts[1]),
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
    Ok(RelatedRanksOutcome {
        columns,
        rows_used: rows.complete() as u64,
        test,
        exclusions,
        notes,
        limitations: LIMITATIONS.to_vec(),
    })
}

fn refuse<B>(code: RelatedRanksCode) -> Halt<B> {
    Halt::Refused(Diagnostic::error(code))
}

/// `v`, or not computable with `overflow` when it is infinite or NaN.
fn stat(v: f64) -> Stat {
    if v.is_finite() {
        Stat::Value(v)
    } else {
        Stat::NotComputable(RelatedRanksCode::Overflow)
    }
}

/// The sum of the sizes' t³ − t, in u128, converted to a double once.
fn tie_sum(sizes: &[u128]) -> f64 {
    sizes.iter().map(|t| t * t * t - t).sum::<u128>() as f64
}

/// One sign's ranks: its count, its sum and its mean, absent when it has none.
fn sign_ranks(ranks: impl Iterator<Item = f64>) -> SignRanks {
    let (mut count, mut rank_sum) = (0u64, 0.0);
    for r in ranks {
        count += 1;
        rank_sum += r;
    }
    SignRanks {
        count,
        rank_sum,
        mean_rank: (count > 0).then(|| rank_sum / count as f64),
    }
}

/// z and its normal p from n > 0, the tie sum and V.
fn z_and_p(n: f64, ties: f64, v: f64) -> (Stat, Stat) {
    let variance = n * (n + 1.0) * (2.0 * n + 1.0) / 24.0 - ties / 48.0;
    let z = stat((v - n * (n + 1.0) / 4.0) / variance.sqrt());
    let p = match z {
        Stat::Value(z) => stat(normal_two_tailed(z)),
        not_computable => not_computable,
    };
    (z, p)
}

/// The exact two-sided p from its tails, each checked first, since `f64::min`
/// turns a NaN into the other operand (decision 635).
fn exact_two_sided(lower: f64, upper: f64) -> Result<f64, RelatedRanksCode> {
    if !(lower.is_finite() && upper.is_finite()) {
        return Err(RelatedRanksCode::Overflow);
    }
    Ok((2.0 * lower.min(upper)).min(1.0))
}

/// The Wilcoxon signed-rank test of `a` less `b`, row by row, and its notes.
fn wilcoxon(
    first: Alias,
    second: Alias,
    a: &[f64],
    b: &[f64],
) -> Result<(Wilcoxon, Vec<RelatedRanksCode>), RelatedRanksCode> {
    let all: Vec<f64> = a.iter().zip(b).map(|(x, y)| x - y).collect();
    let mut sorted = all.clone();
    sorted.sort_by(f64::total_cmp);
    let median_difference = stat(median(&sorted));
    let d: Vec<f64> = all.iter().copied().filter(|d| *d != 0.0).collect();
    let zero_differences = (all.len() - d.len()) as u64;
    let n = d.len() as u64;
    let sizes_of: Vec<f64> = d.iter().map(|x| x.abs()).collect();
    let ranks = average_ranks(&sizes_of);
    let mut sizes = sizes_of;
    sizes.sort_by(f64::total_cmp);
    let tied = runs(&sizes);
    let tie_runs = tied.iter().filter(|t| **t >= 2).count() as u64;
    let negative = sign_ranks(d.iter().zip(&ranks).filter(|(x, _)| **x < 0.0).map(|(_, r)| *r));
    let positive = sign_ranks(d.iter().zip(&ranks).filter(|(x, _)| **x > 0.0).map(|(_, r)| *r));
    let v = positive.rank_sum;
    let big_n = n as f64;
    let top = big_n * (big_n + 1.0) / 2.0;
    let t_minus = top - v;
    let (z, p, rank_biserial) = if n == 0 {
        let nc = Stat::NotComputable(RelatedRanksCode::AllDifferencesZero);
        (nc, nc, nc)
    } else {
        let (z, p) = z_and_p(big_n, tie_sum(&tied), v);
        (z, p, stat((v - t_minus) / top))
    };
    let mut notes = Vec::new();
    if tie_runs > 0 {
        notes.push(RelatedRanksCode::ExactNotGivenTies);
    }
    if zero_differences > 0 {
        notes.push(RelatedRanksCode::ExactNotGivenZeros);
    }
    if n >= EXACT_BELOW {
        notes.push(RelatedRanksCode::ExactNotGivenSize);
    }
    let exact_p = if notes.is_empty() {
        let tails = signed_rank_exact_tails(v as u64, n)
            .ok_or(RelatedRanksCode::DistributionOutOfRange)?;
        Some(exact_two_sided(tails.lower, tails.upper)?)
    } else {
        None
    };
    if zero_differences > 0 {
        notes.push(RelatedRanksCode::ZeroDifferencesDropped);
    }
    if n < ROWS_NOTE_BELOW {
        notes.push(RelatedRanksCode::RowsBelowFive);
    }
    Ok((
        Wilcoxon {
            first,
            second,
            n,
            zero_differences,
            tie_runs,
            negative,
            positive,
            v,
            t_minus,
            z,
            p,
            exact_p,
            rank_biserial,
            median_difference,
        },
        notes,
    ))
}

/// Q, its p and Kendall's W from the deviations' spread and the tie correction.
fn q_p_w(rows: f64, k: f64, spread: f64, correction: f64) -> (Stat, Stat, Stat) {
    let q = stat((12.0 / (rows * k * (k + 1.0))) * spread / correction);
    match q {
        Stat::Value(q) => (
            Stat::Value(q),
            match chi_square_upper(q, k - 1.0) {
                Some(p) => stat(p),
                None => Stat::NotComputable(RelatedRanksCode::DistributionOutOfRange),
            },
            stat(q / (rows * (k - 1.0))),
        ),
        not_computable => (not_computable, not_computable, not_computable),
    }
}

/// The Friedman test of `columns`, each row's values ranked within the row.
fn friedman(columns: &[Alias], values: &[Vec<f64>]) -> Friedman {
    let k = columns.len();
    let rows = values[0].len();
    let mut rank_sums = vec![0.0; k];
    let (mut ties, mut all_tied) = (0u128, true);
    let mut row = vec![0.0; k];
    for r in 0..rows {
        for (cell, column) in row.iter_mut().zip(values) {
            *cell = column[r];
        }
        for (sum, rank) in rank_sums.iter_mut().zip(average_ranks(&row)) {
            *sum += rank;
        }
        let mut sorted = row.clone();
        sorted.sort_by(f64::total_cmp);
        let sizes = runs(&sorted);
        all_tied &= sizes.len() == 1;
        ties += sizes.iter().map(|t| t * t * t - t).sum::<u128>();
    }
    let (n, width) = (rows as f64, k as f64);
    let df = width - 1.0;
    let (q, p, kendalls_w) = if all_tied {
        let nc = Stat::NotComputable(RelatedRanksCode::AllValuesTied);
        (nc, nc, nc)
    } else {
        let centre = n * (width + 1.0) / 2.0;
        let spread: f64 = rank_sums.iter().map(|r| (r - centre) * (r - centre)).sum();
        // Whole numbers to here: 1 − ties / whole in doubles cancels when nearly
        // every row is tied, and this difference does not.
        let whole = rows as u128 * ((k as u128).pow(3) - k as u128);
        q_p_w(n, width, spread, (whole - ties) as f64 / whole as f64)
    };
    Friedman {
        columns: columns
            .iter()
            .zip(&rank_sums)
            .map(|(column, rank_sum)| FriedmanColumn {
                column: *column,
                rank_sum: *rank_sum,
                mean_rank: rank_sum / n,
            })
            .collect(),
        q,
        df,
        p,
        kendalls_w,
    }
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows used
/// are the complete rows; with the exclusions they add up to the data rows. The
/// output hash is SHA-256 of [`RelatedRanksOutcome::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<RelatedRanksParams>,
    dataset: &Dataset,
    result: &RelatedRanksOutcome,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<RelatedRanksCode> {
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

/// Decision 636: what no input reaches, with crafted numbers.
#[cfg(test)]
mod tests {
    use super::*;

    const OVERFLOWED: Stat = Stat::NotComputable(RelatedRanksCode::Overflow);

    #[test]
    fn a_tail_that_is_not_finite_gives_no_exact_p() {
        assert_eq!(exact_two_sided(f64::NAN, 0.25), Err(RelatedRanksCode::Overflow));
        assert_eq!(exact_two_sided(0.25, f64::INFINITY), Err(RelatedRanksCode::Overflow));
        assert_eq!(exact_two_sided(0.25, 0.75), Ok(0.5));
        assert_eq!(exact_two_sided(0.75, 0.75), Ok(1.0));
    }

    #[test]
    fn an_overflow_in_z_or_its_p_is_not_computable() {
        // A tie sum that makes σ² negative: its root is NaN.
        let (z, p) = z_and_p(4.0, 1e6, 3.0);
        assert_eq!((z, p), (OVERFLOWED, OVERFLOWED));
    }

    #[test]
    fn an_overflow_in_q_its_p_or_w_is_not_computable_and_a_df_past_the_range_has_its_own_code() {
        // A correction of exactly 0.
        assert_eq!(q_p_w(4.0, 3.0, 8.0, 0.0), (OVERFLOWED, OVERFLOWED, OVERFLOWED));
        let (q, p, w) = q_p_w(4.0, 3000.0, 8.0, 1.0);
        assert!(matches!(q, Stat::Value(_)), "{q:?}");
        assert_eq!(p, Stat::NotComputable(RelatedRanksCode::DistributionOutOfRange));
        assert!(matches!(w, Stat::Value(_)), "{w:?}");
        // rows 0: W divides by 0 while Q divides by 0 too, so all three overflow.
        assert_eq!(q_p_w(0.0, 3.0, 8.0, 1.0).2, OVERFLOWED);
    }
}
