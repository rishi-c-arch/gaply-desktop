//! **The descriptive summary: the Research OS's first operation (Phase 2A).**
//!
//! * [`profile`] describes each column of a dataset: its kind, n, missing,
//!   distinct values, a categorical column's level counts, and the
//!   candidate missing codes Level 1's rule finds.
//! * [`validate`] checks an `OperationSpec<DescribeParams>` against the
//!   dataset and fills in the estimated rows, columns and cells.
//! * [`describe`] computes, per requested column, n, missing, mean, sd
//!   (n − 1), min, the quartiles by R's default (type 7) and max, or a
//!   categorical column's levels in code-point order with their counts.
//! * [`manifest`] records the run, and [`export`] writes the result as CSV
//!   and the manifest as JSON.
//! * [`describe_until`] is `describe` with a check point before each
//!   column, where the run manager (Phase 2B) stops a cancelled run or one
//!   past its time ceiling.
//!
//! Pure: no file, network, process, clock or model; the caller passes the
//! elapsed time and the platform (`tests/contracts_are_pure.rs`).
//!
//! **One implementation of each formula.** The mean is
//! `stats_verify::mean`, the two-pass mean R uses (evaluation fixes, part C,
//! m-10). The variance is `stats_verify::sample_variance`, over the same
//! mean and `stats_verify`'s range-checked sum of squares. Both are given
//! the values in ascending order, so permuting the rows changes no bit of
//! the result; R sums in row order, and the golden gate holds the
//! difference. Candidate missing codes come from Level 1's own rule,
//! `binding::suspect_codes_in`, and are only reported: a value is missing
//! here only when the user lists it in `missing_codes`, and only in a column
//! requested as Numeric; a Categorical request keeps it as a level (decision
//! 38).
//!
//! **One counting pass,** `tally`, serves profile, validate, describe and
//! manifest, so they cannot disagree on n or missing.
//!
//! **Edge cases are closed outcomes, never NaN or infinity.** n = 0 makes
//! every statistic `no_values`; n = 1 makes sd `single_value`; a column
//! whose min equals its max has sd exactly 0, decided before any
//! arithmetic; a sum, square or root a double cannot hold is `overflow` or
//! `underflow`; a non-finite cell refuses a numeric summary.
//!
//! **Levels** are Level 1's level codes (`Cell::level_code`): a number
//! prints without a trailing point, so `1`, `1.0` and `1.00` are one level,
//! and text is trimmed. R keeps a mixed column's text as written.
//!
//! **Privacy.** `DatasetProfile` and `DescribeResult` hold column headers and
//! level labels, for the user's own screen and the export the user saves;
//! they derive `Serialize` for that and nothing else, and their `Debug`
//! prints counts only. The manifest holds hashes and counts.

pub mod export;

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::ops::ControlFlow;

use serde::de::{Deserializer, Error as _};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use crate::contracts::codes::closed;
use crate::contracts::{
    CapabilityId, ContentHash, Diagnostic, Exclusion, InputRef, OperationId, OperationSpec,
    ParamKey, ParamValue, Platform, Reproducibility, ResourceEstimate, RunManifest, SchemaVersion,
    SemanticRole, ValidationReport, WorkspaceId,
};
use crate::stats_l1::binding::suspect_codes_in;
use crate::stats_l1::dataset::{
    Cell, ColumnAlias, ColumnIssue, ColumnSummary, ColumnType, Dataset,
};
use crate::stats_verify::{mean, sample_variance, RangeFault};

/// The capability's id in the registry Phase 4 builds.
pub const CAPABILITY_ID: &str = "stats.describe";
/// Raised when what a spec means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;

closed! {
    /// Every finding, outcome and exclusion of the descriptive summary.
    pub enum DescCode in "stats.desc." {
        /// The spec names another capability.
        UnknownCapability => "unknown_capability",
        /// The spec was written for another version of this capability.
        CapabilityVersionMismatch => "capability_version_mismatch",
        /// The spec does not name exactly one input.
        InputCount => "input_count",
        /// The spec's input is not this dataset.
        InputMismatch => "input_mismatch",
        /// No column was chosen.
        NoColumnsSelected => "no_columns_selected",
        /// No column has that alias.
        UnknownColumn => "unknown_column",
        /// The column is requested twice.
        DuplicateColumn => "duplicate_column",
        /// A numeric summary was asked for a column holding text.
        NumericOnText => "numeric_on_text",
        /// A numeric summary was asked for a column holding NaN or an infinity.
        NonFiniteCell => "non_finite_cell",
        /// A missing code is NaN or an infinity.
        MissingCodeNotFinite => "missing_code_not_finite",
        /// A candidate missing code the rule found and the user did not list.
        SuspectMissingCode => "suspect_missing_code",
        /// Not computable: the column has no values.
        NoValues => "no_values",
        /// Not computable: sd needs two values.
        SingleValue => "single_value",
        /// Not computable: a sum, square or root overflowed.
        Overflow => "overflow",
        /// Not computable: a sum of squares underflowed though the values differ.
        Underflow => "underflow",
        /// Not computable: the arithmetic refused for a reason it did not name.
        ArithmeticRefused => "arithmetic_refused",
        /// Excluded: an empty cell or a missing-value token.
        MissingCell => "missing_cell",
        /// Excluded: a value the user listed in `missing_codes`.
        MissingCode => "missing_code",
    }
}

/* ------------------------------------------------------------------------ *
 *  Parameters
 * ------------------------------------------------------------------------ */

/// A column alias as Level 1 writes it on the wire: `C1`, `C2`, … in file
/// order. Only `C` followed by a number from 1, with no leading zero, is
/// read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Alias(pub ColumnAlias);

impl Serialize for Alias {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.name())
    }
}

impl<'de> Deserialize<'de> for Alias {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let word = String::deserialize(d)?;
        let digits = word.strip_prefix('C').filter(|n| {
            !n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit())
        });
        digits
            .and_then(|n| n.parse::<u32>().ok())
            .map(|n| Alias(ColumnAlias(n)))
            .ok_or_else(|| D::Error::custom("not a column alias"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreatAs {
    Numeric,
    Categorical,
}

/// One column to summarise, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColumnRequest {
    pub alias: Alias,
    pub treat_as: TreatAs,
}

impl ColumnRequest {
    /// The column as its profile reads it: numeric for a numeric or empty
    /// column, categorical otherwise. The user may change it.
    pub fn default_for(column: &ColumnProfile) -> ColumnRequest {
        let treat_as = match column.kind {
            ColumnKind::Numeric | ColumnKind::Empty => TreatAs::Numeric,
            ColumnKind::Categorical => TreatAs::Categorical,
        };
        ColumnRequest {
            alias: column.alias,
            treat_as,
        }
    }
}

/// The descriptive summary's parameters: the columns, in output order, and
/// the values the user chose to treat as missing in the columns requested as
/// Numeric. Nothing else is missing beyond empty cells and the missing-value
/// tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DescribeParams {
    pub columns: Vec<ColumnRequest>,
    pub missing_codes: Vec<f64>,
}

impl DescribeParams {
    /// Every column, in file order, each as its profile reads it, with no
    /// missing codes.
    pub fn defaults_for(profile: &DatasetProfile) -> DescribeParams {
        DescribeParams {
            columns: profile
                .columns
                .iter()
                .map(ColumnRequest::default_for)
                .collect(),
            missing_codes: Vec::new(),
        }
    }
}

/* ------------------------------------------------------------------------ *
 *  The profile
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    /// Every value a number.
    Numeric,
    /// Text, dates, or numbers mixed with text.
    Categorical,
    /// Every cell missing.
    Empty,
}

/// A level and how many cells hold it. The label is the user's own text.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelCount {
    pub label: String,
    pub count: u64,
}

impl std::fmt::Debug for LevelCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LevelCount")
            .field("label_chars", &self.label.chars().count())
            .field("count", &self.count)
            .finish()
    }
}

/// A candidate missing code that Level 1's rule found in a column, and how
/// many cells hold it. Reported, never applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuspectCode {
    pub code: i32,
    pub cells: u64,
}

/// One column of the profile, for the user's own screen.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnProfile {
    pub alias: Alias,
    /// The original header. Never in a manifest, a log or an outbound struct.
    pub header: String,
    pub kind: ColumnKind,
    /// Non-missing cells.
    pub n: u64,
    pub missing: u64,
    /// Distinct non-missing values: by value for a numeric column, by level
    /// for any other.
    pub distinct: u64,
    /// A categorical column's levels in code-point order; absent for a
    /// numeric or empty column.
    pub levels: Option<Vec<LevelCount>>,
    pub suspect_missing_codes: Vec<SuspectCode>,
    /// Cells holding NaN written out or an infinity.
    pub non_finite: u64,
}

impl std::fmt::Debug for ColumnProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColumnProfile")
            .field("alias", &self.alias)
            .field("header_chars", &self.header.chars().count())
            .field("kind", &self.kind)
            .field("n", &self.n)
            .field("missing", &self.missing)
            .field("distinct", &self.distinct)
            .field("levels", &self.levels.as_ref().map(Vec::len))
            .field("suspect_missing_codes", &self.suspect_missing_codes)
            .field("non_finite", &self.non_finite)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetProfile {
    pub rows: u64,
    pub columns: Vec<ColumnProfile>,
}

/* ------------------------------------------------------------------------ *
 *  The result
 * ------------------------------------------------------------------------ */

/// A statistic, or the closed reason it could not be computed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    Value(f64),
    NotComputable(DescCode),
}

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericSummary {
    pub alias: Alias,
    pub header: String,
    pub n: u64,
    pub missing: u64,
    pub mean: Stat,
    pub sd: Stat,
    pub min: Stat,
    pub q1: Stat,
    pub median: Stat,
    pub q3: Stat,
    pub max: Stat,
}

impl std::fmt::Debug for NumericSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NumericSummary")
            .field("alias", &self.alias)
            .field("header_chars", &self.header.chars().count())
            .field("n", &self.n)
            .field("missing", &self.missing)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoricalSummary {
    pub alias: Alias,
    pub header: String,
    pub n: u64,
    pub missing: u64,
    pub levels: Vec<LevelCount>,
}

impl std::fmt::Debug for CategoricalSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CategoricalSummary")
            .field("alias", &self.alias)
            .field("header_chars", &self.header.chars().count())
            .field("n", &self.n)
            .field("missing", &self.missing)
            .field("levels", &self.levels)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnResult {
    Numeric(NumericSummary),
    Categorical(CategoricalSummary),
}

/// The summary of every requested column, in the request's order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribeResult {
    pub columns: Vec<ColumnResult>,
}

impl DescribeResult {
    /// The canonical bytes of the result: what `output_hash` hashes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a result of numbers, closed words and strings serialises")
    }
}

/* ------------------------------------------------------------------------ *
 *  The one counting pass
 * ------------------------------------------------------------------------ */

/// What one column holds, under the user's missing codes. Never leaves this
/// module, and derives nothing that could print or serialise it.
struct Tally {
    /// The numbers left after the codes, in row order.
    numbers: Vec<f64>,
    /// Non-missing text cells.
    texts: u64,
    /// Empty cells and missing-value tokens.
    missing_cells: u64,
    /// Numbers the user listed in `missing_codes`.
    missing_coded: u64,
    /// Every non-missing level and its count, in code-point order.
    levels: BTreeMap<String, u64>,
}

impl Tally {
    fn n(&self) -> u64 {
        self.numbers.len() as u64 + self.texts
    }
    fn missing(&self) -> u64 {
        self.missing_cells + self.missing_coded
    }
}

fn tally(cells: &[Cell], missing_codes: &[f64]) -> Tally {
    let mut t = Tally {
        numbers: Vec::new(),
        texts: 0,
        missing_cells: 0,
        missing_coded: 0,
        levels: BTreeMap::new(),
    };
    for cell in cells {
        match cell {
            Cell::Missing => {
                t.missing_cells += 1;
                continue;
            }
            Cell::Number(v) if missing_codes.contains(v) => {
                t.missing_coded += 1;
                continue;
            }
            Cell::Number(v) => t.numbers.push(*v),
            Cell::Text(_) => t.texts += 1,
        }
        if let Some(code) = cell.level_code() {
            *t.levels.entry(code).or_insert(0) += 1;
        }
    }
    t
}

/// The missing codes a request leaves out: the listed ones for a Numeric
/// request, none for a Categorical one, whose `-99` stays a level (decision
/// 38, evaluation of Phases 2 and 3, F5). The screen asks about codes in the
/// columns summarised as numeric, and `validate` warns about them there only.
fn codes_for<'a>(request: &ColumnRequest, params: &'a DescribeParams) -> &'a [f64] {
    match request.treat_as {
        TreatAs::Numeric => &params.missing_codes,
        TreatAs::Categorical => &[],
    }
}

fn level_counts(t: &Tally) -> Vec<LevelCount> {
    t.levels
        .iter()
        .map(|(label, count)| LevelCount {
            label: label.clone(),
            count: *count,
        })
        .collect()
}

fn column(dataset: &Dataset, alias: Alias) -> Option<(&ColumnSummary, &[Cell])> {
    Some((
        dataset.schema.column(alias.0)?,
        dataset.column_cells(alias.0)?,
    ))
}

fn kind_of(column: &ColumnSummary) -> ColumnKind {
    match column.column_type {
        ColumnType::Numeric | ColumnType::Integer => ColumnKind::Numeric,
        ColumnType::Empty => ColumnKind::Empty,
        ColumnType::Categorical | ColumnType::Text | ColumnType::Date => ColumnKind::Categorical,
    }
}

fn non_finite_cells(column: &ColumnSummary) -> u64 {
    column
        .issues
        .iter()
        .map(|i| {
            if let ColumnIssue::NonFinite { cells } = i {
                *cells as u64
            } else {
                0
            }
        })
        .sum()
}

fn column_param(alias: Alias) -> ParamValue<DescCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

/* ------------------------------------------------------------------------ *
 *  profile
 * ------------------------------------------------------------------------ */

/// Every column of the dataset, with no missing codes applied.
pub fn profile(dataset: &Dataset) -> DatasetProfile {
    let columns = dataset
        .schema
        .columns
        .iter()
        .map(|summary| {
            let alias = Alias(summary.alias);
            let cells = dataset
                .column_cells(summary.alias)
                .expect("every schema column has cells");
            let t = tally(cells, &[]);
            let kind = kind_of(summary);
            let distinct = match kind {
                ColumnKind::Numeric => {
                    let mut v = t.numbers.clone();
                    v.sort_by(f64::total_cmp);
                    v.dedup_by(|a, b| a == b);
                    v.len() as u64
                }
                ColumnKind::Categorical | ColumnKind::Empty => t.levels.len() as u64,
            };
            ColumnProfile {
                alias,
                header: summary.header.clone(),
                kind,
                n: t.n(),
                missing: t.missing(),
                distinct,
                levels: (kind == ColumnKind::Categorical).then(|| level_counts(&t)),
                suspect_missing_codes: suspect_codes_in(summary.alias, cells)
                    .into_iter()
                    .map(|s| SuspectCode {
                        code: s.code,
                        cells: s.cells as u64,
                    })
                    .collect(),
                non_finite: non_finite_cells(summary),
            }
        })
        .collect();
    DatasetProfile {
        rows: dataset.rows() as u64,
        columns,
    }
}

/* ------------------------------------------------------------------------ *
 *  validate
 * ------------------------------------------------------------------------ */

/// The capability's id, parsed.
pub fn capability_id() -> CapabilityId {
    CapabilityId::parse(CAPABILITY_ID).expect("the capability id is well formed")
}

/// The dataset as an operation's input.
pub fn input_ref(dataset: &Dataset) -> InputRef {
    InputRef {
        content_hash: ContentHash::from_hex(dataset.schema.fingerprint.sha256_hex())
            .expect("a fingerprint is SHA-256 hex"),
        rows: dataset.rows() as u64,
        columns: dataset.schema.columns.len() as u64,
        semantic_role: SemanticRole::Table,
    }
}

/// A spec for this capability on this dataset.
pub fn spec(
    operation_id: OperationId,
    dataset: &Dataset,
    parameters: DescribeParams,
) -> OperationSpec<DescribeParams> {
    OperationSpec {
        schema_version: SchemaVersion,
        operation_id,
        workspace_id: WorkspaceId::Statistics,
        capability_id: capability_id(),
        capability_version: CAPABILITY_VERSION,
        inputs: vec![input_ref(dataset)],
        parameters,
        random_seed: None,
    }
}

/// What is wrong with the parameters for this dataset, in request order:
/// non-finite missing codes first, then per column an unknown alias, a
/// repeat, a non-finite cell or text under a numeric request. Shared by
/// `validate` and `describe`.
fn parameter_errors(dataset: &Dataset, params: &DescribeParams) -> Vec<Diagnostic<DescCode>> {
    let mut errors = Vec::new();
    for (i, code) in params.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(
                Diagnostic::error(DescCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64),
            );
        }
    }
    let mut seen = Vec::new();
    for request in &params.columns {
        let alias = request.alias;
        let Some((summary, cells)) = column(dataset, alias) else {
            errors.push(
                Diagnostic::error(DescCode::UnknownColumn)
                    .with(ParamKey::Column, column_param(alias)),
            );
            continue;
        };
        if seen.contains(&alias) {
            errors.push(
                Diagnostic::error(DescCode::DuplicateColumn)
                    .with(ParamKey::Column, column_param(alias)),
            );
            continue;
        }
        seen.push(alias);
        if request.treat_as == TreatAs::Numeric {
            let t = tally(cells, &params.missing_codes);
            let non_finite = non_finite_cells(summary);
            if non_finite > 0 {
                errors.push(
                    Diagnostic::error(DescCode::NonFiniteCell)
                        .with(ParamKey::Column, column_param(alias))
                        .int(ParamKey::Count, non_finite as i64),
                );
            } else if t.texts > 0 {
                errors.push(
                    Diagnostic::error(DescCode::NumericOnText)
                        .with(ParamKey::Column, column_param(alias))
                        .int(ParamKey::Count, t.texts as i64),
                );
            }
        }
    }
    errors
}

/// Checks the spec against the dataset. Unsupported when it names another
/// capability, version or workspace; invalid on a wrong input or a
/// parameter error; needs input when no column is chosen; a warning for a
/// candidate missing code in a numeric column that the user did not list.
pub fn validate(
    dataset: &Dataset,
    spec: &OperationSpec<DescribeParams>,
) -> ValidationReport<DescCode> {
    let mut unsupported = Vec::new();
    // A spec for another workspace names no capability of this one. The app
    // refuses it first, with `workspace.wrong_workspace` (Phase 3).
    if spec.workspace_id != WorkspaceId::Statistics || spec.capability_id.as_str() != CAPABILITY_ID
    {
        unsupported.push(Diagnostic::error(DescCode::UnknownCapability));
    }
    if spec.capability_version != CAPABILITY_VERSION {
        unsupported.push(
            Diagnostic::error(DescCode::CapabilityVersionMismatch)
                .int(ParamKey::Expected, i64::from(CAPABILITY_VERSION))
                .int(ParamKey::Found, i64::from(spec.capability_version)),
        );
    }

    let mut errors = Vec::new();
    match spec.inputs.as_slice() {
        [only] => {
            if *only != input_ref(dataset) {
                errors.push(Diagnostic::error(DescCode::InputMismatch));
            }
        }
        inputs => errors.push(
            Diagnostic::error(DescCode::InputCount).int(ParamKey::Count, inputs.len() as i64),
        ),
    }
    let params = &spec.parameters;
    errors.extend(parameter_errors(dataset, params));

    let mut missing_information = Vec::new();
    if params.columns.is_empty() {
        missing_information.push(Diagnostic::note(DescCode::NoColumnsSelected));
    }

    let mut warnings = Vec::new();
    for request in params
        .columns
        .iter()
        .filter(|r| r.treat_as == TreatAs::Numeric)
    {
        let Some((_, cells)) = column(dataset, request.alias) else {
            continue;
        };
        for s in suspect_codes_in(request.alias.0, cells) {
            if !params.missing_codes.contains(&f64::from(s.code)) {
                warnings.push(
                    Diagnostic::warning(DescCode::SuspectMissingCode)
                        .with(ParamKey::Column, column_param(request.alias))
                        .int(ParamKey::Value, i64::from(s.code))
                        .int(ParamKey::Count, s.cells as i64),
                );
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
    ValidationReport::new(unsupported, errors, missing_information, warnings, estimate)
}

/* ------------------------------------------------------------------------ *
 *  describe
 * ------------------------------------------------------------------------ */

/// The summary of each requested column, or the first parameter error.
pub fn describe(
    dataset: &Dataset,
    params: &DescribeParams,
) -> Result<DescribeResult, Diagnostic<DescCode>> {
    match describe_until(dataset, params, |_| ControlFlow::<Infallible>::Continue(())) {
        Ok(result) => Ok(result),
        Err(Halt::Refused(e)) => Err(e),
        Err(Halt::Stopped { reason, .. }) => match reason {},
    }
}

/// Why [`describe_until`] returned no result.
#[derive(Debug, Clone, PartialEq)]
pub enum Halt<B> {
    /// The parameters are wrong for this dataset: what `describe` returns.
    Refused(Diagnostic<DescCode>),
    /// The caller's checkpoint stopped the run after `done` columns.
    Stopped { reason: B, done: usize },
}

/// [`describe`], with a check point before each column (Phase 2B).
/// `checkpoint(done)` is called with the number of columns already
/// summarised, before the first column and between every two, and a `Break`
/// stops the run there. What a stop means, a cancel or a time ceiling, is
/// the caller's: this module reads no flag and no clock.
pub fn describe_until<B>(
    dataset: &Dataset,
    params: &DescribeParams,
    mut checkpoint: impl FnMut(usize) -> ControlFlow<B>,
) -> Result<DescribeResult, Halt<B>> {
    if let Some(e) = parameter_errors(dataset, params).into_iter().next() {
        return Err(Halt::Refused(e));
    }
    if params.columns.is_empty() {
        return Err(Halt::Refused(Diagnostic::note(DescCode::NoColumnsSelected)));
    }
    let mut columns = Vec::with_capacity(params.columns.len());
    for (done, request) in params.columns.iter().enumerate() {
        if let ControlFlow::Break(reason) = checkpoint(done) {
            return Err(Halt::Stopped { reason, done });
        }
        let (summary, cells) = column(dataset, request.alias).expect("checked by parameter_errors");
        let t = tally(cells, codes_for(request, params));
        let header = summary.header.clone();
        columns.push(match request.treat_as {
            TreatAs::Numeric => ColumnResult::Numeric(
                numeric_summary(request.alias, header, &t).map_err(Halt::Refused)?,
            ),
            TreatAs::Categorical => ColumnResult::Categorical(CategoricalSummary {
                alias: request.alias,
                header,
                n: t.n(),
                missing: t.missing(),
                levels: level_counts(&t),
            }),
        });
    }
    Ok(DescribeResult { columns })
}

fn finite_or_overflow(v: f64) -> Stat {
    if v.is_finite() {
        Stat::Value(v)
    } else {
        Stat::NotComputable(DescCode::Overflow)
    }
}

/// R's default quantile (type 7) of ascending values: with h = (n − 1)·p,
/// j = ⌊h⌋ and g = h − j, it is (1 − g)·x[j] + g·x[j+1]. As in R, a quantile
/// that falls on a value, or between two equal values, is that value.
fn type7(sorted: &[f64], p: f64) -> f64 {
    let h = (sorted.len() - 1) as f64 * p;
    let j = h.floor();
    let g = h - j;
    let lo = sorted[j as usize];
    if g == 0.0 {
        return lo;
    }
    let hi = sorted[j as usize + 1];
    if hi == lo {
        return lo;
    }
    (1.0 - g) * lo + g * hi
}

fn numeric_summary(
    alias: Alias,
    header: String,
    t: &Tally,
) -> Result<NumericSummary, Diagnostic<DescCode>> {
    let (n, missing) = (t.n(), t.missing());
    let mut sorted = t.numbers.clone();
    if sorted.iter().any(|v| !v.is_finite()) {
        let cells = sorted.iter().filter(|v| !v.is_finite()).count();
        return Err(Diagnostic::error(DescCode::NonFiniteCell)
            .with(ParamKey::Column, column_param(alias))
            .int(ParamKey::Count, cells as i64));
    }
    if sorted.is_empty() {
        let none = Stat::NotComputable(DescCode::NoValues);
        return Ok(NumericSummary {
            alias,
            header,
            n,
            missing,
            mean: none,
            sd: none,
            min: none,
            q1: none,
            median: none,
            q3: none,
            max: none,
        });
    }
    sorted.sort_by(f64::total_cmp);
    let (min, max) = (sorted[0], sorted[sorted.len() - 1]);
    let sd = if sorted.len() == 1 {
        Stat::NotComputable(DescCode::SingleValue)
    } else if min == max {
        Stat::Value(0.0)
    } else {
        match sample_variance(&sorted) {
            Ok(v) => finite_or_overflow(v.sqrt()),
            Err(e) => Stat::NotComputable(match RangeFault::of(&e) {
                Some(RangeFault::Overflow) => DescCode::Overflow,
                Some(RangeFault::Underflow) => DescCode::Underflow,
                None => DescCode::ArithmeticRefused,
            }),
        }
    };
    Ok(NumericSummary {
        alias,
        header,
        n,
        missing,
        mean: finite_or_overflow(mean(&sorted)),
        sd,
        min: Stat::Value(min),
        q1: finite_or_overflow(type7(&sorted, 0.25)),
        median: finite_or_overflow(type7(&sorted, 0.5)),
        q3: finite_or_overflow(type7(&sorted, 0.75)),
        max: Stat::Value(max),
    })
}

/* ------------------------------------------------------------------------ *
 *  manifest
 * ------------------------------------------------------------------------ */

/// The record of a run of `spec` on `dataset` that produced `result`. Rows
/// read and used are the data rows: this capability excludes cells, never a
/// whole row, and each column's missing cells and coded values are listed
/// as exclusions. The output hash is SHA-256 of
/// [`DescribeResult::canonical_bytes`].
pub fn manifest(
    spec: &OperationSpec<DescribeParams>,
    dataset: &Dataset,
    result: &DescribeResult,
    elapsed_ms: u64,
    platform: Platform,
) -> RunManifest<DescCode> {
    let mut exclusions = Vec::new();
    for request in &spec.parameters.columns {
        let Some((_, cells)) = column(dataset, request.alias) else {
            continue;
        };
        let t = tally(cells, codes_for(request, &spec.parameters));
        for (code, count) in [
            (DescCode::MissingCell, t.missing_cells),
            (DescCode::MissingCode, t.missing_coded),
        ] {
            if count > 0 {
                let params = BTreeMap::from([(ParamKey::Column, column_param(request.alias))]);
                exclusions.push(Exclusion {
                    code,
                    count,
                    params,
                });
            }
        }
    }
    let parameters = serde_json::to_vec(&spec.parameters)
        .expect("parameters of aliases, words and numbers serialise");
    let rows = dataset.rows() as u64;
    RunManifest {
        schema_version: SchemaVersion,
        capability_id: capability_id(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        inputs: spec.inputs.iter().map(|i| i.content_hash.clone()).collect(),
        parameters_hash: ContentHash::of_bytes(&parameters),
        seed: spec.random_seed,
        rows_read: rows,
        rows_used: rows,
        exclusions,
        platform,
        elapsed_ms,
        output_hash: ContentHash::of_bytes(&result.canonical_bytes()),
        reproducibility: Reproducibility::Bitwise,
    }
}
