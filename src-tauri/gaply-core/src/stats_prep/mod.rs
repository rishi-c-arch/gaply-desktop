//! **Scale scores: a new numeric column made from items the user chooses
//! (Research OS, Phase 8M, decision 408; ADR-011): `stats.prepare.scale_score`.**
//!
//! The user chooses the items, any to reverse on the scale's ends they type,
//! the sum or the mean, a rule for missing answers (every item has a value, or
//! at least n do) and a name. Gaply keeps those steps as a [`Recipe`] and
//! applies them; it never changes the file. Nothing here chooses, proposes or
//! advises: a statistic or a rule left open needs input.
//!
//! **The score.** Per row, an item's value is a number that is not a ticked
//! missing code, reversed as min + max − x when the item is reversed. When
//! the rule holds, the score is the sum of those values in column order, or
//! their mean. Otherwise, and when a sum or mean does not fit a double, the
//! cell is empty, never a code. Four counts say what happened to the rows.
//!
//! **One formula, one place (decision 406).** The reversal, the count of
//! values outside the ends and the row total are `stats_alpha`'s; the mean is
//! `stats_verify::mean`. No arithmetic is written here. The build uses only
//! addition, subtraction and division (ADR-011 Part 7).
//!
//! **A score's identity (ADR-011 Part 4).** Its column is numbered on from
//! the file's, `C(k + j)`, and `Dataset::with_appended_columns` sets its whole
//! summary: its header is the alias word, its kind `Numeric`, never levels. So
//! the name the user typed can never reach a schema, and through it a result.
//! The name lives in the recipe alone. A score's items are the file's columns:
//! a score of scores would make the recipe a graph (ADR-011 question 13).
//!
//! **The prepared input (ADR-011 Part 5).** A prepared table's fingerprint is
//! SHA-256 over the original's hash and the recipe's, not over its cells: what
//! was asked for, so a last-digit difference between platforms does not look
//! like different data, and two recipes are two inputs.
//!
//! **One number, one spelling (decision 416).** -0 as a scale end or a missing
//! code is refused: it equals 0 and has other bytes, so it would make one
//! recipe two hashes.
//!
//! **Limits.** The columns against `MAX_COLUMNS`, which counts the scores, so
//! a 128-column file takes no score; the cells against a cap the caller
//! passes, so this module holds no tier (ADR-011 Part 9). Both refuse, with no
//! allowance.
//!
//! **Privacy.** A refusal holds closed codes and numbers. `Debug` prints
//! counts, never a name or a value (ADR-011 Part 11). This module writes
//! nothing: [`export`] returns bytes.

use serde::{Deserialize, Serialize};

use crate::contracts::codes::closed;
use crate::contracts::{
    CapabilityId, Diagnostic, ParamKey, ParamValue, ResourceEstimate, ValidationReport,
    ValidationStatus,
};
use crate::research_project::is_invisible_format;
use crate::stats_alpha::{outside_scale_count, reverse, row_total, ScaleEnds};
use crate::stats_desc::Alias;
use crate::stats_l1::dataset::{
    sha256_hex, AppendRefusal, AppendedCell, Cell, DataFingerprint, Dataset, DatasetRefusal,
    MAX_COLUMNS,
};
use crate::stats_ttest::{column, non_finite_cells};
use crate::stats_verify::mean;

pub mod export;

/// The capability's id in the registry.
pub const CAPABILITY_ID: &str = "stats.prepare.scale_score";
/// Raised when what a recipe means changes.
pub const CAPABILITY_VERSION: u32 = 1;
/// Raised when any number this module computes may change.
pub const ENGINE_VERSION: u32 = 1;
/// The recipe's shape.
pub const RECIPE_SCHEMA_VERSION: u32 = 1;
/// The longest name, in characters.
pub const NAME_MAX_CHARS: usize = 128;
/// The fewest items a score may have. The most is what [`MAX_COLUMNS`] leaves
/// for the score's own column.
pub const MIN_ITEMS: usize = 2;

closed! {
    /// Every refusal, note and count of scale scores.
    pub enum PrepCode in "stats.prep." {
        /// The recipe's bytes are not a recipe in its one canonical form.
        RecipeUnreadable => "recipe_unreadable",
        /// The recipe names another capability.
        UnknownCapability => "unknown_capability",
        /// The recipe was written for another version of this capability.
        /// Params `expected`, `found`.
        CapabilityVersionMismatch => "capability_version_mismatch",
        /// The recipe was written for another engine. Params `expected`,
        /// `found`.
        EngineVersionMismatch => "engine_version_mismatch",
        /// The recipe has another shape. Params `expected`, `found`.
        SchemaVersionMismatch => "schema_version_mismatch",
        /// The table already holds prepared columns; a recipe is applied to
        /// the file's table.
        SourceAlreadyPrepared => "source_already_prepared",
        /// The recipe was written for a file of another width. Params
        /// `expected`, `found`.
        SourceColumnsMismatch => "source_columns_mismatch",
        /// The recipe has no steps.
        NoSteps => "no_steps",
        /// A step's column is not the next after the file's. Params `index`,
        /// `expected`, `found`.
        AliasNotNext => "alias_not_next",
        /// Past the column limit, the scores included. Params `found`, `limit`.
        TooWide => "too_wide",
        /// Past this computer's cell limit, the scores included. Params
        /// `found`, `limit`.
        CellsOverCap => "cells_over_cap",
        /// The name has whitespace at an end. Param `index`.
        NameNotTrimmed => "name_not_trimmed",
        /// The name is empty or too long. Params `index`, `count`.
        NameLength => "name_length",
        /// The name holds a control or invisible character. Param `index`.
        NameHasInvisibleCharacter => "name_has_invisible_character",
        /// The name is one of the file's headers. Params `index`, `column`.
        NameEqualsFileHeader => "name_equals_file_header",
        /// The name is an earlier score's. Params `index`, `from` (that
        /// score's index).
        NameEqualsOtherScore => "name_equals_other_score",
        /// The name is spelled as a column's alias. Param `index`.
        NameIsAnAliasWord => "name_is_an_alias_word",
        /// Fewer than two items. Params `index`, `count`.
        TooFewItems => "too_few_items",
        /// An item is listed twice. Params `index`, `column`.
        ItemRepeated => "item_repeated",
        /// The items are not in column order; the first out of order. Params
        /// `index`, `column`.
        ItemsOutOfOrder => "items_out_of_order",
        /// An item is not one of the file's columns. Params `index`, `column`.
        ItemNotAFileColumn => "item_not_a_file_column",
        /// An item holds no numbers. Params `index`, `column`.
        ItemNotNumeric => "item_not_numeric",
        /// An item holds text. Params `index`, `column`, `count`.
        ItemHoldsText => "item_holds_text",
        /// An item holds NaN or an infinity. Params `index`, `column`, `count`.
        ItemNonFinite => "item_non_finite",
        /// A reversed item is listed twice. Params `index`, `column`.
        ReversedRepeated => "reversed_repeated",
        /// The reversed items are not in column order. Params `index`,
        /// `column`.
        ReversedOutOfOrder => "reversed_out_of_order",
        /// A reversed item is not among the items. Params `index`, `column`.
        ReversedNotAnItem => "reversed_not_an_item",
        /// An item is reversed and the scale's ends are absent. Param `index`.
        ScaleMissing => "scale_missing",
        /// The scale's ends are given and nothing is reversed. Param `index`.
        ScaleWithoutReversed => "scale_without_reversed",
        /// A scale's end is NaN or an infinity. Param `index`.
        ScaleNotFinite => "scale_not_finite",
        /// The scale's lowest value is not below its highest. Param `index`.
        ScaleEndsOutOfOrder => "scale_ends_out_of_order",
        /// A reversed item has values outside the ends, ticked codes removed.
        /// Params `index`, `column`, `count`.
        ReversedOutsideScale => "reversed_outside_scale",
        /// "At least n" is outside 1 to k − 1. Params `index`, `found`, `limit`.
        AtLeastOutOfRange => "at_least_out_of_range",
        /// A missing code is NaN or an infinity. Param `index` (the code's).
        MissingCodeNotFinite => "missing_code_not_finite",
        /// A missing code is listed twice. Param `index` (the code's).
        MissingCodeRepeated => "missing_code_repeated",
        /// The missing codes are not ascending. Param `index` (the code's).
        MissingCodesOutOfOrder => "missing_codes_out_of_order",
        /// Needs input: the sum or the mean has not been chosen. Param `index`.
        StatisticNotChosen => "statistic_not_chosen",
        /// Needs input: the rule for missing answers has not been chosen.
        /// Param `index`.
        RuleNotChosen => "rule_not_chosen",
        /// A scale end or a missing code is -0, which would make one number
        /// two spellings and so two recipes (decision 416). A screen sends
        /// "-0" as 0. Param `index` (the step's).
        NegativeZero => "negative_zero",
        /// Refused when made: no row has this score. Param `column`, with the
        /// four counts beside it.
        NoRowScored => "no_row_scored",
        /// The table refused the new column for a reason this module checks
        /// first; reaching it is a defect. Param `column`.
        TableRefusedColumn => "table_refused_column",
        /// Note: a sum under "at least n" is not on the same scale as a full
        /// sum. Param `index`.
        SumOverFewerItems => "sum_over_fewer_items",
        /// Count: rows scored from every item.
        ScoredFromEveryItem => "scored_from_every_item",
        /// Count: rows scored from fewer than every item.
        ScoredFromFewer => "scored_from_fewer",
        /// Count: rows not scored, too few items have a value.
        NotScoredTooFew => "not_scored_too_few",
        /// Count: rows not scored, the sum or mean does not fit a double.
        NotScoredOverflow => "not_scored_overflow",
    }
}

/* ------------------------------------------------------------------------ *
 *  The recipe
 * ------------------------------------------------------------------------ */

/// What a score is, of the values a row has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Statistic {
    Sum,
    Mean,
}

/// When a row has a score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingRule {
    /// Every item has a value.
    EveryItem,
    /// At least this many do, from 1 to k − 1.
    AtLeast(u32),
}

/// One score. The statistic and the rule have no default: each is open until
/// the user chooses (ADR-011 Part 1). `name` is the one text in a recipe.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScaleScoreStep {
    /// This score's column, `C(k + j)`.
    pub alias: Alias,
    /// As the user typed it; never in a schema, a result or a log.
    pub name: String,
    /// The file's columns, in column order.
    pub items: Vec<Alias>,
    /// A subset of `items`, in the same order.
    pub reversed: Vec<Alias>,
    /// Required when an item is reversed, absent otherwise.
    pub scale: Option<ScaleEnds>,
    pub statistic: Option<Statistic>,
    pub rule: Option<MissingRule>,
    /// Ascending; each applies by value to every item.
    pub missing_codes: Vec<f64>,
}

/// Counts only, never the name, the ends or a code.
impl std::fmt::Debug for ScaleScoreStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScaleScoreStep")
            .field("alias", &self.alias)
            .field("items", &self.items.len())
            .field("reversed", &self.reversed.len())
            .field("missing_codes", &self.missing_codes.len())
            .finish_non_exhaustive()
    }
}

/// A step of a recipe, tagged by its kind, so a later kind is a new tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecipeStep {
    ScaleScore(ScaleScoreStep),
}

/// The steps that make a prepared table from a file's.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Recipe {
    pub schema_version: u32,
    pub capability_id: CapabilityId,
    pub capability_version: u32,
    pub engine_version: u32,
    /// How many columns the file has.
    pub source_columns: u32,
    pub steps: Vec<RecipeStep>,
}

/// Counts only.
impl std::fmt::Debug for Recipe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recipe")
            .field("source_columns", &self.source_columns)
            .field("steps", &self.steps.len())
            .finish_non_exhaustive()
    }
}

impl Recipe {
    /// A recipe from its canonical bytes, and nothing else: unknown fields,
    /// other spacing or order and absent fields are refused, so one recipe has
    /// one set of bytes and so one hash. The closed code echoes nothing.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Recipe, PrepCode> {
        let recipe: Recipe =
            serde_json::from_slice(bytes).map_err(|_| PrepCode::RecipeUnreadable)?;
        if recipe_bytes(&recipe) == bytes {
            Ok(recipe)
        } else {
            Err(PrepCode::RecipeUnreadable)
        }
    }

    fn scores(&self) -> impl Iterator<Item = &ScaleScoreStep> {
        self.steps.iter().map(|s| match s {
            RecipeStep::ScaleScore(step) => step,
        })
    }
}

/// The recipe's canonical bytes: compact JSON, its fields in declared order.
pub fn recipe_bytes(recipe: &Recipe) -> Vec<u8> {
    serde_json::to_vec(recipe).expect("a recipe of numbers, aliases and a name serialises")
}

/// SHA-256 of [`recipe_bytes`], lowercase hex.
pub fn recipe_hash(recipe: &Recipe) -> String {
    sha256_hex(&recipe_bytes(recipe))
}

/// What a prepared input's hash is taken over (ADR-011 Part 5):
/// `gaply.prepared.v1\n`, the original bytes' SHA-256, `\n` and the recipe's
/// SHA-256, each lowercase hex, with no trailing newline. The formula's one
/// place: [`prepared_input_hash`] and [`prepared_fingerprint`] both read it.
fn prepared_input_bytes(source_sha256_hex: &str, recipe_sha256_hex: &str) -> String {
    format!("gaply.prepared.v1\n{source_sha256_hex}\n{recipe_sha256_hex}")
}

/// A prepared input's SHA-256, lowercase hex, from the two hashes alone, so a
/// method can check a spec's preparation with no recipe in hand (decision 417).
pub fn prepared_input_hash(source_sha256_hex: &str, recipe_sha256_hex: &str) -> String {
    sha256_hex(prepared_input_bytes(source_sha256_hex, recipe_sha256_hex).as_bytes())
}

/// A prepared table's fingerprint: [`prepared_input_hash`]'s hash, with the
/// source's parser version and table reference.
pub fn prepared_fingerprint(source: &DataFingerprint, recipe: &Recipe) -> DataFingerprint {
    DataFingerprint::of_bytes(
        prepared_input_bytes(source.sha256_hex(), &recipe_hash(recipe)).as_bytes(),
        source.parser_version(),
        source.table().clone(),
    )
}

/* ------------------------------------------------------------------------ *
 *  validate
 * ------------------------------------------------------------------------ */

fn column_param(alias: Alias) -> ParamValue<PrepCode> {
    ParamValue::Int(i64::from(alias.0 .0))
}

/// A diagnostic for step `index`.
fn at(code: PrepCode, index: usize) -> Diagnostic<PrepCode> {
    Diagnostic::error(code).int(ParamKey::Index, index as i64)
}

fn at_column(code: PrepCode, index: usize, alias: Alias) -> Diagnostic<PrepCode> {
    at(code, index).with(ParamKey::Column, column_param(alias))
}

fn version_check(code: PrepCode, expected: u32, found: u32, out: &mut Vec<Diagnostic<PrepCode>>) {
    if expected != found {
        out.push(
            Diagnostic::error(code)
                .int(ParamKey::Expected, i64::from(expected))
                .int(ParamKey::Found, i64::from(found)),
        );
    }
}

/// `C` followed by a number from 1 with no leading zero: the form an alias
/// has on the wire, whether or not this table has that column.
fn is_alias_word(name: &str) -> bool {
    name.strip_prefix('C').is_some_and(|n| {
        !n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit())
    })
}

fn order_errors(
    list: &[Alias],
    repeated: PrepCode,
    out_of_order: PrepCode,
    index: usize,
    errors: &mut Vec<Diagnostic<PrepCode>>,
) {
    for w in list.windows(2) {
        if w[0] == w[1] {
            errors.push(at_column(repeated, index, w[1]));
        } else if w[0] > w[1] {
            errors.push(at_column(out_of_order, index, w[1]));
        }
    }
}

/// The name's own rules, then its clashes, each giving one finding.
fn name_errors(
    dataset: &Dataset,
    scores: &[&ScaleScoreStep],
    index: usize,
    errors: &mut Vec<Diagnostic<PrepCode>>,
) {
    let name = scores[index].name.as_str();
    let chars = name.chars().count();
    if chars == 0 || chars > NAME_MAX_CHARS {
        errors.push(at(PrepCode::NameLength, index).int(ParamKey::Count, chars as i64));
        return;
    }
    if name.trim() != name {
        errors.push(at(PrepCode::NameNotTrimmed, index));
        return;
    }
    if name
        .chars()
        .any(|c| c.is_control() || is_invisible_format(c))
    {
        errors.push(at(PrepCode::NameHasInvisibleCharacter, index));
        return;
    }
    if is_alias_word(name) {
        errors.push(at(PrepCode::NameIsAnAliasWord, index));
        return;
    }
    // After trimming both sides, as `build_dataset` trims headers.
    if let Some(c) = dataset
        .schema
        .columns
        .iter()
        .find(|c| c.header.trim() == name)
    {
        errors.push(
            at(PrepCode::NameEqualsFileHeader, index).int(ParamKey::Column, i64::from(c.alias.0)),
        );
        return;
    }
    if let Some(first) = scores[..index].iter().position(|s| s.name.trim() == name) {
        errors.push(at(PrepCode::NameEqualsOtherScore, index).int(ParamKey::From, first as i64));
    }
}

/// -0.0, which equals 0.0 and has other bytes.
fn is_negative_zero(v: f64) -> bool {
    v == 0.0 && v.is_sign_negative()
}

/// The items, the reversed items, the scale, the rule and the codes of step
/// `index`, in that order, one choice one form (decision 338's rule).
fn step_errors(
    dataset: &Dataset,
    step: &ScaleScoreStep,
    index: usize,
    errors: &mut Vec<Diagnostic<PrepCode>>,
) {
    let start = errors.len();
    for (i, code) in step.missing_codes.iter().enumerate() {
        if !code.is_finite() {
            errors.push(
                Diagnostic::error(PrepCode::MissingCodeNotFinite).int(ParamKey::Index, i as i64),
            );
        }
    }
    let scale_ends = step.scale.iter().flat_map(|s| [s.min, s.max]);
    if step
        .missing_codes
        .iter()
        .copied()
        .chain(scale_ends)
        .any(is_negative_zero)
    {
        errors.push(at(PrepCode::NegativeZero, index));
    }
    for (i, w) in step.missing_codes.windows(2).enumerate() {
        if w[0] == w[1] {
            errors.push(
                Diagnostic::error(PrepCode::MissingCodeRepeated).int(ParamKey::Index, i as i64 + 1),
            );
        } else if w[0] > w[1] {
            errors.push(
                Diagnostic::error(PrepCode::MissingCodesOutOfOrder)
                    .int(ParamKey::Index, i as i64 + 1),
            );
        }
    }
    let items = &step.items;
    if items.len() < MIN_ITEMS {
        errors.push(at(PrepCode::TooFewItems, index).int(ParamKey::Count, items.len() as i64));
    }
    order_errors(
        items,
        PrepCode::ItemRepeated,
        PrepCode::ItemsOutOfOrder,
        index,
        errors,
    );
    order_errors(
        &step.reversed,
        PrepCode::ReversedRepeated,
        PrepCode::ReversedOutOfOrder,
        index,
        errors,
    );
    for alias in &step.reversed {
        if !items.contains(alias) {
            errors.push(at_column(PrepCode::ReversedNotAnItem, index, *alias));
        }
    }
    match (step.scale, step.reversed.is_empty()) {
        (Some(_), true) => errors.push(at(PrepCode::ScaleWithoutReversed, index)),
        (None, false) => errors.push(at(PrepCode::ScaleMissing, index)),
        (Some(s), false) if !(s.min.is_finite() && s.max.is_finite()) => {
            errors.push(at(PrepCode::ScaleNotFinite, index))
        }
        (Some(s), false) if s.min >= s.max => errors.push(at(PrepCode::ScaleEndsOutOfOrder, index)),
        _ => {}
    }
    if let Some(MissingRule::AtLeast(n)) = step.rule {
        let top = items.len().saturating_sub(1);
        if n == 0 || n as usize > top {
            errors.push(
                at(PrepCode::AtLeastOutOfRange, index)
                    .int(ParamKey::Found, i64::from(n))
                    .int(ParamKey::Limit, top as i64),
            );
        }
    }
    for alias in items {
        if alias.0.index() >= dataset.file_columns() {
            errors.push(at_column(PrepCode::ItemNotAFileColumn, index, *alias));
            continue;
        }
        let Some((summary, cells)) = column(dataset, *alias) else {
            errors.push(at_column(PrepCode::ItemNotAFileColumn, index, *alias));
            continue;
        };
        let non_finite = non_finite_cells(summary);
        let texts = cells.iter().filter(|c| matches!(c, Cell::Text(_))).count();
        if non_finite > 0 {
            errors.push(
                at_column(PrepCode::ItemNonFinite, index, *alias)
                    .int(ParamKey::Count, non_finite as i64),
            );
        } else if texts > 0 {
            errors.push(
                at_column(PrepCode::ItemHoldsText, index, *alias)
                    .int(ParamKey::Count, texts as i64),
            );
        } else if !summary.is_numeric() {
            errors.push(at_column(PrepCode::ItemNotNumeric, index, *alias));
        }
    }
    if errors.len() > start {
        return;
    }
    // A reversed item's values outside the scale, in every row read, ticked
    // codes removed.
    if let Some(ends) = step.scale {
        for alias in &step.reversed {
            if let Some((_, cells)) = column(dataset, *alias) {
                let outside = outside_scale_count(cells, ends, &step.missing_codes);
                if outside > 0 {
                    errors.push(
                        at_column(PrepCode::ReversedOutsideScale, index, *alias)
                            .int(ParamKey::Count, outside as i64),
                    );
                }
            }
        }
    }
}

/// Checks the recipe against the file's table: Unsupported when it was written
/// for another capability, version, engine or shape; Invalid on an error;
/// NeedsUserInput when a statistic or rule is open; a note for a sum under
/// "at least n". `cells_cap` is this computer's limit, from the caller.
pub fn validate(dataset: &Dataset, recipe: &Recipe, cells_cap: u64) -> ValidationReport<PrepCode> {
    let mut unsupported = Vec::new();
    if recipe.capability_id.as_str() != CAPABILITY_ID {
        unsupported.push(Diagnostic::error(PrepCode::UnknownCapability));
    }
    version_check(
        PrepCode::CapabilityVersionMismatch,
        CAPABILITY_VERSION,
        recipe.capability_version,
        &mut unsupported,
    );
    version_check(
        PrepCode::EngineVersionMismatch,
        ENGINE_VERSION,
        recipe.engine_version,
        &mut unsupported,
    );
    version_check(
        PrepCode::SchemaVersionMismatch,
        RECIPE_SCHEMA_VERSION,
        recipe.schema_version,
        &mut unsupported,
    );

    let mut errors = Vec::new();
    let file_columns = dataset.file_columns();
    if dataset.schema.columns.len() != file_columns {
        errors.push(Diagnostic::error(PrepCode::SourceAlreadyPrepared));
    }
    version_check(
        PrepCode::SourceColumnsMismatch,
        file_columns as u32,
        recipe.source_columns,
        &mut errors,
    );
    if recipe.steps.is_empty() {
        errors.push(Diagnostic::error(PrepCode::NoSteps));
    }
    let width = file_columns + recipe.steps.len();
    if width > MAX_COLUMNS {
        errors.push(
            Diagnostic::error(PrepCode::TooWide)
                .int(ParamKey::Found, width as i64)
                .int(ParamKey::Limit, MAX_COLUMNS as i64),
        );
    }
    let cells = dataset.rows() as u64 * width as u64;
    if cells > cells_cap {
        errors.push(
            Diagnostic::error(PrepCode::CellsOverCap)
                .int(ParamKey::Found, cells as i64)
                .int(ParamKey::Limit, cells_cap as i64),
        );
    }

    let mut questions = Vec::new();
    let mut warnings = Vec::new();
    let scores: Vec<&ScaleScoreStep> = recipe.scores().collect();
    for (index, step) in scores.iter().enumerate() {
        let expected = file_columns as u32 + index as u32 + 1;
        if step.alias.0 .0 != expected {
            errors.push(
                at(PrepCode::AliasNotNext, index)
                    .int(ParamKey::Expected, i64::from(expected))
                    .int(ParamKey::Found, i64::from(step.alias.0 .0)),
            );
        }
        name_errors(dataset, &scores, index, &mut errors);
        step_errors(dataset, step, index, &mut errors);
        if step.statistic.is_none() {
            questions.push(at(PrepCode::StatisticNotChosen, index));
        }
        if step.rule.is_none() {
            questions.push(at(PrepCode::RuleNotChosen, index));
        }
        if step.statistic == Some(Statistic::Sum)
            && matches!(step.rule, Some(MissingRule::AtLeast(_)))
        {
            warnings.push(
                Diagnostic::warning(PrepCode::SumOverFewerItems).int(ParamKey::Index, index as i64),
            );
        }
    }
    ValidationReport::new(
        unsupported,
        errors,
        questions,
        warnings,
        ResourceEstimate {
            rows: dataset.rows() as u64,
            columns: width as u64,
            cells,
        },
    )
}

/* ------------------------------------------------------------------------ *
 *  build
 * ------------------------------------------------------------------------ */

/// What happened to the rows of one score. Counts only, no values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreCounts {
    pub column: Alias,
    pub scored_from_every_item: u64,
    pub scored_from_fewer: u64,
    pub not_scored_too_few: u64,
    pub not_scored_overflow: u64,
}

impl ScoreCounts {
    /// The four counts under their closed words.
    pub fn by_code(&self) -> [(PrepCode, u64); 4] {
        [
            (PrepCode::ScoredFromEveryItem, self.scored_from_every_item),
            (PrepCode::ScoredFromFewer, self.scored_from_fewer),
            (PrepCode::NotScoredTooFew, self.not_scored_too_few),
            (PrepCode::NotScoredOverflow, self.not_scored_overflow),
        ]
    }

    fn scored(&self) -> u64 {
        self.scored_from_every_item + self.scored_from_fewer
    }
}

/// A prepared table, the recipe that made it, and each score's counts.
pub struct Prepared {
    pub dataset: Dataset,
    pub recipe: Recipe,
    pub counts: Vec<ScoreCounts>,
}

/// Counts only: never a cell, a header or a name.
impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared")
            .field("rows", &self.dataset.rows())
            .field("columns", &self.dataset.schema.columns.len())
            .field("counts", &self.counts)
            .finish_non_exhaustive()
    }
}

/// Why [`build`] returned no table.
#[derive(Debug, Clone, PartialEq)]
pub enum PrepRefusal {
    /// The recipe is unsupported, invalid or open: what [`validate`] says.
    Report(ValidationReport<PrepCode>),
    /// No row has this score (ADR-011 Part 4).
    NoRowScored { column: Alias, counts: ScoreCounts },
    /// The table refused the new column.
    Table(Diagnostic<PrepCode>),
}

/// One score's column and counts. Validation has refused text and non-finite
/// cells, and chosen the statistic and the rule.
fn score_column(
    dataset: &Dataset,
    step: &ScaleScoreStep,
    statistic: Statistic,
    rule: MissingRule,
) -> (Vec<AppendedCell>, ScoreCounts) {
    let k = step.items.len();
    let items: Vec<(&[Cell], Option<ScaleEnds>)> = step
        .items
        .iter()
        .filter_map(|alias| {
            let ends = step.scale.filter(|_| step.reversed.contains(alias));
            column(dataset, *alias).map(|(_, cells)| (cells, ends))
        })
        .collect();
    let needed = match rule {
        MissingRule::EveryItem => k,
        MissingRule::AtLeast(n) => n as usize,
    };
    let mut counts = ScoreCounts {
        column: step.alias,
        scored_from_every_item: 0,
        scored_from_fewer: 0,
        not_scored_too_few: 0,
        not_scored_overflow: 0,
    };
    let mut values = Vec::with_capacity(k);
    let cells = (0..dataset.rows())
        .map(|r| {
            values.clear();
            for (cells, ends) in &items {
                if let Cell::Number(v) = cells[r] {
                    if !step.missing_codes.contains(&v) {
                        values.push(reverse(v, *ends));
                    }
                }
            }
            if values.len() < needed {
                counts.not_scored_too_few += 1;
                return AppendedCell::EMPTY;
            }
            let score = match statistic {
                Statistic::Sum => row_total(values.iter().copied()),
                Statistic::Mean => mean(&values),
            };
            match AppendedCell::number(score) {
                None => {
                    counts.not_scored_overflow += 1;
                    AppendedCell::EMPTY
                }
                Some(cell) => {
                    if values.len() == k {
                        counts.scored_from_every_item += 1;
                    } else {
                        counts.scored_from_fewer += 1;
                    }
                    cell
                }
            }
        })
        .collect();
    (cells, counts)
}

/// The prepared table: the file's columns unchanged, then each score, under
/// [`prepared_fingerprint`]. Writes nothing and leaves `dataset` as it was.
pub fn build(dataset: &Dataset, recipe: &Recipe, cells_cap: u64) -> Result<Prepared, PrepRefusal> {
    let report = validate(dataset, recipe, cells_cap);
    if !matches!(
        report.status,
        ValidationStatus::Valid | ValidationStatus::ValidWithWarnings
    ) {
        return Err(PrepRefusal::Report(report));
    }
    let mut columns = Vec::with_capacity(recipe.steps.len());
    let mut all_counts = Vec::with_capacity(recipe.steps.len());
    for step in recipe.scores() {
        // Validation has refused an open choice.
        let (Some(statistic), Some(rule)) = (step.statistic, step.rule) else {
            return Err(PrepRefusal::Report(report));
        };
        let (cells, counts) = score_column(dataset, step, statistic, rule);
        if counts.scored() == 0 {
            return Err(PrepRefusal::NoRowScored {
                column: step.alias,
                counts,
            });
        }
        columns.push(cells);
        all_counts.push(counts);
    }
    let next = dataset
        .with_appended_columns(
            &columns,
            prepared_fingerprint(&dataset.schema.fingerprint, recipe),
        )
        .map_err(|r| {
            PrepRefusal::Table(match r {
                AppendRefusal::Table(DatasetRefusal::TooWide { columns, limit }) => {
                    Diagnostic::error(PrepCode::TooWide)
                        .int(ParamKey::Found, columns as i64)
                        .int(ParamKey::Limit, limit as i64)
                }
                AppendRefusal::Table(DatasetRefusal::NoColumns)
                | AppendRefusal::WrongLength { .. } => {
                    Diagnostic::error(PrepCode::TableRefusedColumn)
                }
            })
        })?;
    Ok(Prepared {
        dataset: next,
        recipe: recipe.clone(),
        counts: all_counts,
    })
}
