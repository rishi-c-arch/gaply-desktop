//! **The dataset as Level 1 sees it: a fingerprint, a typed schema, and the
//! cells, all local.**
//!
//! The app crate's ingest reads the file and hands over a [`RawTable`] of
//! strings with whatever metadata the format carried. This module types each
//! column, buckets its levels and builds the two views of the schema:
//!
//! * [`SchemaSummary`], for this device: aliases AND headers, level codes and
//!   labels, row counts, every issue. Not `Serialize`.
//! * [`AliasedSchema`], the only form that may leave the device: alias, type,
//!   has-missing and level bucket per column. Nothing else, by construction.
//!
//! # Missing values
//!
//! A cell is missing when the shared token list says so
//! ([`crate::stats_verify::is_missing`]: empty, `NA`, `N/A`, `NaN`, `NULL`,
//! `NONE`, `.`, `#N/A`), or when the source's own user-missing definitions
//! cover it (SPSS). So `NaN` is a missing cell, as it is for the existing
//! verifier; an infinity is not on that list and is a non-finite number, which
//! is a typed state ([`ColumnIssue::NonFinite`]) that keeps the column out of
//! every numeric role. Nothing is ever recoded: a `-99` that the file does not
//! declare missing stays a number here, and `binding::confirm` flags it for
//! the user instead.
//!
//! # Every value a cell can take is read through `str::parse::<f64>`
//!
//! That is the opposite of Level 0's rule, and deliberately so. A printed
//! number in a manuscript is an interval its digits allow, so Level 0 reads it
//! exactly. A data cell is the value the analysis software would read, so
//! Level 1 reads it the way the software does.

use serde::Serialize;

use crate::stats_verify::is_missing;

/// The version of this schema builder, recorded in every fingerprint beside
/// the ingest parser's version.
pub const SCHEMA_VERSION: &str = "stats_l1 schema 1";

/// Mirrors the ingest's column cap. A wider table never reaches a binding.
pub const MAX_COLUMNS: usize = 128;
/// A categorical column, or an integer column with at most this many distinct
/// values, is a group column. Level codes and labels are read only up to here.
pub const GROUP_LEVEL_LIMIT: usize = 50;
/// A text column with more distinct values than this is recorded as a
/// category explosion: not a group, and not free text either.
pub const CATEGORY_EXPLOSION_ABOVE: usize = 1_000;

/* ------------------------------------------------------------------------ *
 *  What the ingest hands over
 * ------------------------------------------------------------------------ */

/// A user-missing definition from the source file (SPSS). Codes the author
/// declared, verbatim and typed.
#[derive(Debug, Clone, PartialEq)]
pub enum MissingRule {
    Value(f64),
    Range { lo: f64, hi: f64 },
    Text(String),
}

impl MissingRule {
    fn covers(&self, cell: &str) -> bool {
        match self {
            MissingRule::Value(v) => parse_number(cell).is_some_and(|x| x == *v),
            MissingRule::Range { lo, hi } => parse_number(cell).is_some_and(|x| x >= *lo && x <= *hi),
            MissingRule::Text(t) => cell.trim() == t.trim(),
        }
    }
}

/// What the source told us about one column, beyond its cells. Empty for a
/// format that carries nothing (CSV, a spreadsheet).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawVariable {
    pub user_missing: Vec<MissingRule>,
    /// `code -> label`, where `code` is the value as the cells print it.
    pub value_labels: Vec<(String, String)>,
    /// `Some(true)` when the format declares the column a string variable.
    pub declared_text: Option<bool>,
}

/// Whether the source format's variable metadata (user-missing, value labels,
/// a weight) was read. Three states, because "none" and "could not read" are
/// different facts about a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMetadata {
    /// The format has no such metadata (CSV, spreadsheet, MATLAB).
    NotCarried,
    /// The format has it and it was read (SPSS).
    Read,
    /// The format has it and ingest could not read it. Treated as absent, and
    /// said so.
    NotKnown,
}

/// The ingest's output, as strings: one header per column, one string per
/// cell, in file order. Never a path or a file name.
#[derive(Debug, Clone, PartialEq)]
pub struct RawTable {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// One per header, or empty when the format carries nothing.
    pub variables: Vec<RawVariable>,
    /// The declared weight variable's header, when the format declares one.
    pub weight_variable: Option<String>,
    pub metadata: SourceMetadata,
}

impl RawTable {
    /// A table from headers and rows and nothing else: a CSV.
    pub fn from_rows(headers: Vec<String>, rows: Vec<Vec<String>>) -> RawTable {
        RawTable { headers, rows, variables: Vec::new(), weight_variable: None, metadata: SourceMetadata::NotCarried }
    }
}

/* ------------------------------------------------------------------------ *
 *  The fingerprint
 * ------------------------------------------------------------------------ */

/// Which table of the file was read. A file's bytes alone do not say which
/// sheet the schema came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableRef {
    /// The first (and only) table of a CSV.
    FirstTable,
    Sheet { index: usize, name: String },
    SpssCases,
    MatlabVariables,
}

/// SHA-256 over the exact file bytes, with the parser that read them and the
/// table it read. Never a path: the same bytes under another name are the
/// same data. Not `Serialize`: a fingerprint never leaves the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFingerprint {
    sha256_hex: String,
    parser_version: String,
    table: TableRef,
}

impl DataFingerprint {
    pub fn of_bytes(bytes: &[u8], parser_version: &str, table: TableRef) -> DataFingerprint {
        DataFingerprint { sha256_hex: sha256_hex(bytes), parser_version: parser_version.to_string(), table }
    }

    pub fn sha256_hex(&self) -> &str {
        &self.sha256_hex
    }
    pub fn parser_version(&self) -> &str {
        &self.parser_version
    }
    pub fn table(&self) -> &TableRef {
        &self.table
    }

    /// One string that changes when any part changes: what a token is bound to.
    pub fn digest(&self) -> String {
        sha256_hex(format!("{}|{}|{}|{:?}", self.sha256_hex, self.parser_version, SCHEMA_VERSION, self.table).as_bytes())
    }
}

/// SHA-256, lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/* ------------------------------------------------------------------------ *
 *  The typed schema
 * ------------------------------------------------------------------------ */

/// A column's alias: `C1`, `C2`, … in file order. The only name for a column
/// that may leave the device, and the name every binding uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnAlias(pub u32);

impl ColumnAlias {
    /// Zero-based. `C0` is not an alias and never matches a column.
    pub fn index(self) -> usize {
        (self.0 as usize).saturating_sub(1)
    }
    fn from_index(i: usize) -> ColumnAlias {
        ColumnAlias(i as u32 + 1)
    }
    /// `C7`.
    pub fn name(self) -> String {
        format!("C{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    Numeric,
    Integer,
    Categorical,
    Text,
    Date,
    Empty,
}

impl ColumnType {
    fn wire_name(self) -> &'static str {
        match self {
            ColumnType::Numeric => "numeric",
            ColumnType::Integer => "integer",
            ColumnType::Categorical => "categorical",
            ColumnType::Text => "text",
            ColumnType::Date => "date",
            ColumnType::Empty => "empty",
        }
    }
}

/// How many distinct levels a group column has, bucketed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelBucket {
    Two,
    ThreeToTen,
    MoreThanTen,
}

impl LevelBucket {
    fn of(distinct: usize) -> Option<LevelBucket> {
        match distinct {
            0 | 1 => None,
            2 => Some(LevelBucket::Two),
            3..=10 => Some(LevelBucket::ThreeToTen),
            _ => Some(LevelBucket::MoreThanTen),
        }
    }
    fn wire_name(self) -> &'static str {
        match self {
            LevelBucket::Two => "two",
            LevelBucket::ThreeToTen => "three_to_ten",
            LevelBucket::MoreThanTen => "more_than_ten",
        }
    }
}

/// One level of a group column, for the screen on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelInfo {
    /// The value as the column prints it: `1`, `2`, `control`.
    pub code: String,
    /// The source's value label for the code, when it has one.
    pub label: Option<String>,
    pub count: usize,
}

/// The levels of a group column. Local only: never serialised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalLevels {
    pub levels: Vec<LevelInfo>,
}

impl LocalLevels {
    pub fn codes(&self) -> impl Iterator<Item = &str> {
        self.levels.iter().map(|l| l.code.as_str())
    }
}

/// A typed state about one column. Aliases and counts only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnIssue {
    /// The header is empty after trimming.
    EmptyHeader,
    /// The same header as an earlier column.
    DuplicateHeader { first: ColumnAlias },
    /// Every cell is missing.
    AllMissing,
    /// Some cells are numbers and some are text. The column is typed by the
    /// text, so it cannot be bound as numeric.
    MixedTypes { numbers: usize, texts: usize },
    /// `NaN`, `inf` or `Infinity` in a cell. The column cannot be bound as
    /// numeric.
    NonFinite { cells: usize },
    /// A text column with more than [`CATEGORY_EXPLOSION_ABOVE`] distinct
    /// values.
    CategoryExplosion { distinct: usize },
    /// Rows shorter than the header were padded with missing cells.
    RaggedRows { short_rows: usize },
    /// The source declared user-missing codes and they were applied.
    UserMissingApplied { cells: usize },
}

/// A typed state about the whole table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatasetIssue {
    /// The source declares a weight variable. Level 1 gives `CouldNotCheck`
    /// for a weighted file (Step 4); recorded here so the screen can say so.
    WeightVariableDeclared { column: ColumnAlias },
    /// The source declares a weight variable that is not among the columns.
    WeightVariableUnknown,
    /// The format carries variable metadata and ingest could not read it.
    SourceMetadataNotKnown,
    /// Rows longer than the header: the extra cells were dropped.
    OverlongRows { rows: usize },
    /// A CSV that read as ONE column whose cells hold `;` or a tab: most
    /// likely separated by that character (a semicolon CSV, often with
    /// decimal commas). Not parsed as such: the screen asks for a
    /// comma-separated file or .xlsx (Step 8b, rule 3).
    LooksSeparatedBy { separator: Separator },
}

/// The character a one-column CSV appears to be separated by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Separator {
    Semicolon,
    Tab,
}

/// Rule 3: a CSV read as one column, whose header and most of whose lines
/// hold `;` (or failing that a tab). A line is its fields joined with `,`
/// again: a semicolon file with decimal commas (`-3,55;0,46`) splits at the
/// commas into overlong rows, and the semicolons are in the line, not in its
/// first field.
fn looks_separated(raw: &RawTable, table: &TableRef) -> Option<Separator> {
    if *table != TableRef::FirstTable || raw.headers.len() != 1 {
        return None;
    }
    let lines: Vec<String> = raw.headers.iter().cloned().chain(raw.rows.iter().map(|r| r.join(","))).collect();
    let filled: Vec<&str> = lines.iter().map(String::as_str).filter(|c| !c.trim().is_empty()).collect();
    [(';', Separator::Semicolon), ('\t', Separator::Tab)].into_iter().find_map(|(ch, sep)| {
        let with = filled.iter().filter(|c| c.contains(ch)).count();
        (with > 0 && with * 2 >= filled.len()).then_some(sep)
    })
}

/// Why a file did not become a dataset. No path, no name, no parser text.
/// Moved from the app crate's `stats_l1_ingest` in Phase 2, Step 4, so a
/// Level 1 verdict can carry it; the app crate re-exports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L1IngestRefusal {
    /// The file could not be stat'ed or read.
    NotReadable,
    /// Over `import_guard`'s byte ceiling or the ingest's own size cap.
    TooLarge { bytes: u64, limit: u64 },
    /// Not a format the ingest turns into a table.
    UnsupportedFormat,
    /// The parser could not read the file.
    Malformed,
    /// The file parsed and holds no table.
    NoTable,
    /// More rows than the ingest cap. Level 1 does not compute on a
    /// truncated table.
    FileTooLarge { limit_rows: usize },
    /// The bytes changed between the hash and the parse.
    FileChangedWhileReading,
    /// A `.xls` that is not a readable compound document: an older Excel
    /// format (BIFF2 to BIFF5) or a damaged file. The screen asks for .xlsx
    /// (Step 8b). Told apart by the parser's typed error, never its text.
    XlsNotReadable,
    /// The parsed table could not become a schema.
    Dataset(DatasetRefusal),
}

/// Why no schema was built at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatasetRefusal {
    TooWide { columns: usize, limit: usize },
    NoColumns,
}

/// One column of the schema, for this device.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSummary {
    pub alias: ColumnAlias,
    /// The original header. Never leaves the device.
    pub header: String,
    pub column_type: ColumnType,
    pub has_missing: bool,
    /// Distinct non-missing values, counted for categorical columns and for
    /// integer columns with at most [`GROUP_LEVEL_LIMIT`] of them.
    pub distinct: Option<usize>,
    pub level_bucket: Option<LevelBucket>,
    /// Codes and labels, for the same columns `distinct` is counted on.
    pub levels: Option<LocalLevels>,
    pub issues: Vec<ColumnIssue>,
}

impl ColumnSummary {
    /// Numeric or integer: what a "numeric" role slot accepts.
    pub fn is_numeric(&self) -> bool {
        matches!(self.column_type, ColumnType::Numeric | ColumnType::Integer)
    }
    /// Categorical, or integer with at most [`GROUP_LEVEL_LIMIT`] distinct
    /// values: what a "group" role slot accepts.
    pub fn is_group(&self) -> bool {
        match self.column_type {
            ColumnType::Categorical => true,
            ColumnType::Integer => self.distinct.is_some_and(|d| d <= GROUP_LEVEL_LIMIT),
            _ => false,
        }
    }
}

/// The schema of one loaded dataset. Local only.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaSummary {
    pub fingerprint: DataFingerprint,
    pub columns: Vec<ColumnSummary>,
    pub rows: usize,
    pub weight_variable: Option<ColumnAlias>,
    pub source_metadata: SourceMetadata,
    pub issues: Vec<DatasetIssue>,
}

impl SchemaSummary {
    pub fn column(&self, alias: ColumnAlias) -> Option<&ColumnSummary> {
        self.columns.get(alias.index()).filter(|c| c.alias == alias)
    }

    /// The outbound form: alias, type, has-missing and level bucket, nothing
    /// else.
    pub fn aliased(&self) -> AliasedSchema {
        AliasedSchema {
            columns: self
                .columns
                .iter()
                .map(|c| AliasedColumn {
                    alias: c.alias.name(),
                    column_type: c.column_type.wire_name(),
                    has_missing: c.has_missing,
                    levels: c.level_bucket.map(LevelBucket::wire_name),
                })
                .collect(),
        }
    }
}

/* ------------------------------------------------------------------------ *
 *  The one outbound form
 * ------------------------------------------------------------------------ */

/// The only form of the schema that may leave the device. Four fields per
/// column and no others: no header, no label, no code, no count of rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AliasedSchema {
    pub columns: Vec<AliasedColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AliasedColumn {
    pub alias: String,
    #[serde(rename = "type")]
    pub column_type: &'static str,
    pub has_missing: bool,
    /// `two`, `three_to_ten`, `more_than_ten`, or absent for a column that is
    /// not a group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<&'static str>,
}

/* ------------------------------------------------------------------------ *
 *  The cells
 * ------------------------------------------------------------------------ */

/// One typed cell. Local only.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Missing,
    Number(f64),
    Text(String),
}

impl Cell {
    /// The value as a level code: an integer-valued number prints without a
    /// point, so `1`, `1.0` and `1.00` are one level.
    pub fn level_code(&self) -> Option<String> {
        match self {
            Cell::Missing => None,
            Cell::Number(v) => Some(number_code(*v)),
            Cell::Text(t) => Some(t.clone()),
        }
    }
    pub fn number(&self) -> Option<f64> {
        match self {
            Cell::Number(v) => Some(*v),
            _ => None,
        }
    }
    pub fn is_missing(&self) -> bool {
        matches!(self, Cell::Missing)
    }
}

/// The schema and every cell, column-major. Local only.
#[derive(Debug, Clone, PartialEq)]
pub struct Dataset {
    pub schema: SchemaSummary,
    columns: Vec<Vec<Cell>>,
}

impl Dataset {
    pub fn column_cells(&self, alias: ColumnAlias) -> Option<&[Cell]> {
        self.schema.column(alias).map(|_| self.columns[alias.index()].as_slice())
    }
    pub fn rows(&self) -> usize {
        self.schema.rows
    }

    /// Tests only: put a cell the typing pass never produces (a non-finite
    /// number) where a check that guards against it can be reached.
    #[cfg(test)]
    pub(crate) fn replace_cell_for_test(&mut self, alias: ColumnAlias, row: usize, cell: Cell) {
        self.columns[alias.index()][row] = cell;
    }
}

/* ------------------------------------------------------------------------ *
 *  Building
 * ------------------------------------------------------------------------ */

/// `2.0` prints as `2`; `2.5` as `2.5`. The code a numeric level is known by.
pub fn number_code(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// `str::parse::<f64>` after trimming, accepting a leading `+`. `NaN` and
/// infinities parse, and the caller decides what they mean.
fn parse_number(cell: &str) -> Option<f64> {
    let t = cell.trim();
    if t.is_empty() {
        return None;
    }
    // `parse` accepts "nan", "inf", "infinity" in any case; it does not accept
    // thousands separators, a percent sign or a trailing point, which is right.
    t.parse::<f64>().ok()
}

/// `YYYY-MM-DD`, optionally followed by a time, or calamine's rendered
/// `DateTime(…)` / `DateTimeIso(…)` cell. No calendar check: a column of these
/// is a date column, and a date column is not a number.
fn looks_like_date(cell: &str) -> bool {
    let t = cell.trim();
    if t.starts_with("DateTime(") || t.starts_with("DateTimeIso(") || t.starts_with("DurationIso(") {
        return true;
    }
    let b = t.as_bytes();
    b.len() >= 10
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && (b.len() == 10 || matches!(b[10], b'T' | b' '))
}

struct TypedColumn {
    cells: Vec<Cell>,
    column_type: ColumnType,
    has_missing: bool,
    distinct: Option<usize>,
    levels: Option<LocalLevels>,
    issues: Vec<ColumnIssue>,
}

fn type_column(raw_cells: &[String], var: Option<&RawVariable>) -> TypedColumn {
    let mut cells = Vec::with_capacity(raw_cells.len());
    let (mut numbers, mut texts, mut non_finite, mut dates, mut user_missing_applied) = (0, 0, 0, 0, 0);
    let mut integral = true;
    for raw in raw_cells {
        if is_missing(raw) {
            cells.push(Cell::Missing);
            continue;
        }
        if var.is_some_and(|v| v.user_missing.iter().any(|r| r.covers(raw))) {
            user_missing_applied += 1;
            cells.push(Cell::Missing);
            continue;
        }
        let declared_text = var.and_then(|v| v.declared_text).unwrap_or(false);
        match (declared_text, parse_number(raw)) {
            (false, Some(v)) if v.is_finite() => {
                numbers += 1;
                if v.fract() != 0.0 || v.abs() >= 9.0e15 {
                    integral = false;
                }
                cells.push(Cell::Number(v));
            }
            (false, Some(_)) => {
                non_finite += 1;
                cells.push(Cell::Text(raw.trim().to_string()));
            }
            _ => {
                if looks_like_date(raw) {
                    dates += 1;
                }
                texts += 1;
                cells.push(Cell::Text(raw.trim().to_string()));
            }
        }
    }
    let has_missing = cells.iter().any(Cell::is_missing);
    let mut issues = Vec::new();
    if user_missing_applied > 0 {
        issues.push(ColumnIssue::UserMissingApplied { cells: user_missing_applied });
    }

    let present = numbers + texts + non_finite;
    let column_type = if present == 0 {
        issues.push(ColumnIssue::AllMissing);
        ColumnType::Empty
    } else if non_finite > 0 {
        issues.push(ColumnIssue::NonFinite { cells: non_finite });
        ColumnType::Text
    } else if numbers > 0 && texts > 0 {
        issues.push(ColumnIssue::MixedTypes { numbers, texts });
        if distinct_codes(&cells).len() <= GROUP_LEVEL_LIMIT {
            ColumnType::Categorical
        } else {
            ColumnType::Text
        }
    } else if numbers > 0 {
        if integral {
            ColumnType::Integer
        } else {
            ColumnType::Numeric
        }
    } else if dates == texts {
        ColumnType::Date
    } else {
        let d = distinct_codes(&cells).len();
        if d <= GROUP_LEVEL_LIMIT {
            ColumnType::Categorical
        } else {
            if d > CATEGORY_EXPLOSION_ABOVE {
                issues.push(ColumnIssue::CategoryExplosion { distinct: d });
            }
            ColumnType::Text
        }
    };

    // Levels: categorical always; integer only within the limit.
    let (distinct, levels) = match column_type {
        ColumnType::Categorical => {
            let lv = level_infos(&cells, var);
            (Some(lv.levels.len()), Some(lv))
        }
        ColumnType::Integer => {
            let d = distinct_codes(&cells).len();
            if d <= GROUP_LEVEL_LIMIT {
                (Some(d), Some(level_infos(&cells, var)))
            } else {
                (Some(d), None)
            }
        }
        _ => (None, None),
    };

    TypedColumn { cells, column_type, has_missing, distinct, levels, issues }
}

fn distinct_codes(cells: &[Cell]) -> Vec<String> {
    let mut codes: Vec<String> = cells.iter().filter_map(Cell::level_code).collect();
    codes.sort_unstable();
    codes.dedup();
    codes
}

/// Levels in code order: numeric codes by value, text codes by string.
fn level_infos(cells: &[Cell], var: Option<&RawVariable>) -> LocalLevels {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for code in cells.iter().filter_map(Cell::level_code) {
        match counts.iter_mut().find(|(c, _)| *c == code) {
            Some((_, n)) => *n += 1,
            None => counts.push((code, 1)),
        }
    }
    counts.sort_by(|(a, _), (b, _)| match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        _ => a.cmp(b),
    });
    let label_for = |code: &str| -> Option<String> {
        let labels = &var?.value_labels;
        labels.iter().find(|(c, _)| canonical_code(c) == code).map(|(_, l)| l.clone())
    };
    LocalLevels {
        levels: counts.into_iter().map(|(code, count)| LevelInfo { label: label_for(&code), code, count }).collect(),
    }
}

/// A value-label key as the cells would print it.
fn canonical_code(code: &str) -> String {
    match parse_number(code) {
        Some(v) if v.is_finite() => number_code(v),
        _ => code.trim().to_string(),
    }
}

/// Type every column and build the schema. Refuses only a table too wide to
/// bind or with no columns; every other state is recorded on the column or
/// the table.
pub fn build_dataset(raw: RawTable, fingerprint: DataFingerprint) -> Result<Dataset, DatasetRefusal> {
    let width = raw.headers.len();
    if width == 0 {
        return Err(DatasetRefusal::NoColumns);
    }
    if width > MAX_COLUMNS {
        return Err(DatasetRefusal::TooWide { columns: width, limit: MAX_COLUMNS });
    }

    // Rectangular, column-major strings. Short rows are padded with missing;
    // long rows are cut at the header, and both are counted.
    let mut raw_columns: Vec<Vec<String>> = vec![Vec::with_capacity(raw.rows.len()); width];
    let (mut short_rows, mut long_rows) = (0, 0);
    for row in &raw.rows {
        if row.len() < width {
            short_rows += 1;
        } else if row.len() > width {
            long_rows += 1;
        }
        for (i, col) in raw_columns.iter_mut().enumerate() {
            col.push(row.get(i).cloned().unwrap_or_default());
        }
    }

    let mut columns = Vec::with_capacity(width);
    let mut cells = Vec::with_capacity(width);
    for (i, header) in raw.headers.iter().enumerate() {
        let var = raw.variables.get(i);
        let typed = type_column(&raw_columns[i], var);
        let mut issues = typed.issues;
        let trimmed = header.trim();
        if trimmed.is_empty() {
            issues.push(ColumnIssue::EmptyHeader);
        } else if let Some(first) = raw.headers[..i].iter().position(|h| h.trim() == trimmed) {
            issues.push(ColumnIssue::DuplicateHeader { first: ColumnAlias::from_index(first) });
        }
        if short_rows > 0 {
            issues.push(ColumnIssue::RaggedRows { short_rows });
        }
        columns.push(ColumnSummary {
            alias: ColumnAlias::from_index(i),
            header: header.clone(),
            column_type: typed.column_type,
            has_missing: typed.has_missing,
            distinct: typed.distinct,
            level_bucket: typed.distinct.filter(|d| typed.levels.is_some() && *d <= GROUP_LEVEL_LIMIT).and_then(LevelBucket::of),
            levels: typed.levels,
            issues,
        });
        cells.push(typed.cells);
    }

    let mut issues = Vec::new();
    if let Some(separator) = looks_separated(&raw, &fingerprint.table) {
        issues.push(DatasetIssue::LooksSeparatedBy { separator });
    }
    if long_rows > 0 {
        issues.push(DatasetIssue::OverlongRows { rows: long_rows });
    }
    if raw.metadata == SourceMetadata::NotKnown {
        issues.push(DatasetIssue::SourceMetadataNotKnown);
    }
    let weight_variable = match &raw.weight_variable {
        None => None,
        Some(w) => match raw.headers.iter().position(|h| h == w) {
            Some(i) => {
                let alias = ColumnAlias::from_index(i);
                issues.push(DatasetIssue::WeightVariableDeclared { column: alias });
                Some(alias)
            }
            None => {
                issues.push(DatasetIssue::WeightVariableUnknown);
                None
            }
        },
    };

    let rows = raw.rows.len();
    tracing::debug!(rows, columns = width, "stats_l1: dataset typed");
    Ok(Dataset {
        schema: SchemaSummary { fingerprint, columns, rows, weight_variable, source_metadata: raw.metadata, issues },
        columns: cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp() -> DataFingerprint {
        DataFingerprint::of_bytes(b"x", "test parser", TableRef::FirstTable)
    }

    fn table(headers: &[&str], rows: &[&[&str]]) -> RawTable {
        RawTable::from_rows(
            headers.iter().map(|h| h.to_string()).collect(),
            rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
        )
    }

    fn col<'a>(d: &'a Dataset, i: u32) -> &'a ColumnSummary {
        d.schema.column(ColumnAlias(i)).expect("column")
    }

    #[test]
    fn the_fingerprint_matches_a_known_vector_and_one_byte_changes_it() {
        // SHA-256("abc"), the FIPS 180-2 example vector.
        let f = DataFingerprint::of_bytes(b"abc", "p", TableRef::FirstTable);
        assert_eq!(f.sha256_hex(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let g = DataFingerprint::of_bytes(b"abd", "p", TableRef::FirstTable);
        assert_ne!(f.sha256_hex(), g.sha256_hex());
        assert_ne!(f.digest(), g.digest());
        // The same bytes read from another sheet are another fingerprint.
        let h = DataFingerprint::of_bytes(b"abc", "p", TableRef::Sheet { index: 1, name: "Data".into() });
        assert_eq!(f.sha256_hex(), h.sha256_hex());
        assert_ne!(f.digest(), h.digest());
    }

    #[test]
    fn columns_are_typed_numeric_integer_categorical_text_date_and_empty() {
        let d = build_dataset(
            table(
                &["score", "group", "site", "note", "when", "blank"],
                &[
                    &["1.5", "1", "a", "first visit", "2024-01-03", ""],
                    &["2", "2", "b", "second visit", "2024-02-10", "NA"],
                    &["3.25", "1", "a", "third", "2024-03-11T10:00", "."],
                ],
            ),
            fp(),
        )
        .unwrap();
        assert_eq!(col(&d, 1).column_type, ColumnType::Numeric);
        assert_eq!(col(&d, 2).column_type, ColumnType::Integer);
        assert_eq!(col(&d, 3).column_type, ColumnType::Categorical);
        assert_eq!(col(&d, 4).column_type, ColumnType::Categorical, "three distinct strings are still a group");
        assert_eq!(col(&d, 5).column_type, ColumnType::Date);
        assert_eq!(col(&d, 6).column_type, ColumnType::Empty);
        assert!(col(&d, 6).issues.contains(&ColumnIssue::AllMissing));
        assert!(col(&d, 6).has_missing);
        assert!(!col(&d, 1).has_missing);
        // Integer-coded group: two levels, bucketed, with codes.
        assert_eq!(col(&d, 2).level_bucket, Some(LevelBucket::Two));
        assert_eq!(col(&d, 2).levels.as_ref().unwrap().codes().collect::<Vec<_>>(), vec!["1", "2"]);
        assert!(col(&d, 2).is_group() && col(&d, 2).is_numeric());
        assert!(col(&d, 3).is_group() && !col(&d, 3).is_numeric());
    }

    #[test]
    fn the_same_bytes_give_the_same_schema() {
        let t = table(&["a", "b"], &[&["1", "x"], &["2", "y"]]);
        let d1 = build_dataset(t.clone(), fp()).unwrap();
        let d2 = build_dataset(t, fp()).unwrap();
        assert_eq!(d1, d2);
    }

    #[test]
    fn the_aliased_schema_carries_exactly_four_fields_and_no_header() {
        let d = build_dataset(table(&["Secret Header", "g"], &[&["1", "a"], &["2", "b"], &["", "a"]]), fp()).unwrap();
        let json = serde_json::to_string(&d.schema.aliased()).unwrap();
        assert_eq!(
            json,
            r#"{"columns":[{"alias":"C1","type":"integer","has_missing":true,"levels":"two"},{"alias":"C2","type":"categorical","has_missing":false,"levels":"two"}]}"#
        );
        assert!(!json.contains("Secret"));
    }

    #[test]
    fn user_missing_rules_make_cells_missing_and_value_labels_name_levels() {
        let mut raw = table(&["group", "score"], &[&["1", "10"], &["2", "99"], &["9", "12"], &["1", "-1"]]);
        raw.variables = vec![
            RawVariable {
                user_missing: vec![MissingRule::Value(9.0)],
                value_labels: vec![("1.0".into(), "control".into()), ("2.0".into(), "treatment".into())],
                declared_text: Some(false),
            },
            RawVariable { user_missing: vec![MissingRule::Range { lo: 99.0, hi: 999.0 }], ..Default::default() },
        ];
        raw.metadata = SourceMetadata::Read;
        let d = build_dataset(raw, fp()).unwrap();
        let g = col(&d, 1);
        assert_eq!(g.levels.as_ref().unwrap().codes().collect::<Vec<_>>(), vec!["1", "2"], "9 is missing, not a level");
        assert_eq!(g.levels.as_ref().unwrap().levels[0].label.as_deref(), Some("control"));
        assert!(g.issues.contains(&ColumnIssue::UserMissingApplied { cells: 1 }));
        let s = col(&d, 2);
        assert!(s.has_missing);
        assert_eq!(d.column_cells(ColumnAlias(2)).unwrap()[1], Cell::Missing);
        // -1 is NOT declared missing and is not recoded.
        assert_eq!(d.column_cells(ColumnAlias(2)).unwrap()[3], Cell::Number(-1.0));
    }

    #[test]
    fn a_declared_weight_variable_is_recorded_on_the_table() {
        let mut raw = table(&["w", "y"], &[&["1.5", "3"]]);
        raw.weight_variable = Some("w".into());
        raw.metadata = SourceMetadata::Read;
        let d = build_dataset(raw, fp()).unwrap();
        assert_eq!(d.schema.weight_variable, Some(ColumnAlias(1)));
        assert!(d.schema.issues.contains(&DatasetIssue::WeightVariableDeclared { column: ColumnAlias(1) }));
        let mut raw = table(&["w", "y"], &[&["1.5", "3"]]);
        raw.weight_variable = Some("gone".into());
        let d = build_dataset(raw, fp()).unwrap();
        assert!(d.schema.issues.contains(&DatasetIssue::WeightVariableUnknown));
    }

    #[test]
    fn metadata_the_ingest_could_not_read_is_said_so() {
        let mut raw = table(&["a"], &[&["1"]]);
        raw.metadata = SourceMetadata::NotKnown;
        let d = build_dataset(raw, fp()).unwrap();
        assert!(d.schema.issues.contains(&DatasetIssue::SourceMetadataNotKnown));
    }

    #[test]
    fn duplicate_and_empty_headers_are_typed_states() {
        let d = build_dataset(table(&["a", " ", "a"], &[&["1", "2", "3"]]), fp()).unwrap();
        assert!(col(&d, 2).issues.contains(&ColumnIssue::EmptyHeader));
        assert!(col(&d, 3).issues.contains(&ColumnIssue::DuplicateHeader { first: ColumnAlias(1) }));
        assert!(col(&d, 1).issues.is_empty());
    }

    #[test]
    fn mixed_types_non_finite_and_ragged_rows_are_typed_states() {
        let d = build_dataset(
            table(&["m", "inf", "nan"], &[&["1", "1", "1"], &["two", "Infinity", "NaN"], &["3", "3"]]),
            fp(),
        )
        .unwrap();
        assert!(col(&d, 1).issues.contains(&ColumnIssue::MixedTypes { numbers: 2, texts: 1 }));
        assert_eq!(col(&d, 1).column_type, ColumnType::Categorical);
        assert!(!col(&d, 1).is_numeric());
        assert!(col(&d, 2).issues.contains(&ColumnIssue::NonFinite { cells: 1 }));
        assert_eq!(col(&d, 2).column_type, ColumnType::Text);
        // `NaN` is a MISSING token on the shared list the existing verifier
        // uses, so it is missing here too, not a non-finite number.
        assert_eq!(col(&d, 3).column_type, ColumnType::Integer);
        assert!(col(&d, 3).has_missing);
        assert_eq!(d.column_cells(ColumnAlias(3)).unwrap()[1], Cell::Missing);
        assert!(col(&d, 3).issues.contains(&ColumnIssue::RaggedRows { short_rows: 1 }));
        assert_eq!(d.column_cells(ColumnAlias(3)).unwrap()[2], Cell::Missing);
        // `-inf` is non-finite.
        let d = build_dataset(table(&["neg"], &[&["1"], &["-inf"], &["2"]]), fp()).unwrap();
        assert!(col(&d, 1).issues.contains(&ColumnIssue::NonFinite { cells: 1 }));
    }

    #[test]
    fn two_thousand_columns_are_refused_and_a_ten_thousand_level_column_is_an_explosion() {
        let headers: Vec<String> = (0..2000).map(|i| format!("c{i}")).collect();
        let raw = RawTable::from_rows(headers, vec![]);
        assert_eq!(build_dataset(raw, fp()), Err(DatasetRefusal::TooWide { columns: 2000, limit: MAX_COLUMNS }));
        assert_eq!(build_dataset(RawTable::from_rows(vec![], vec![]), fp()), Err(DatasetRefusal::NoColumns));

        let rows: Vec<Vec<String>> = (0..10_000).map(|i| vec![format!("id{i}")]).collect();
        let d = build_dataset(RawTable::from_rows(vec!["id".into()], rows), fp()).unwrap();
        assert_eq!(col(&d, 1).column_type, ColumnType::Text);
        assert!(col(&d, 1).issues.contains(&ColumnIssue::CategoryExplosion { distinct: 10_000 }));
        assert!(!col(&d, 1).is_group());
    }

    #[test]
    fn an_integer_column_with_more_than_fifty_values_is_numeric_not_a_group() {
        let rows: Vec<Vec<String>> = (0..60).map(|i| vec![i.to_string()]).collect();
        let d = build_dataset(RawTable::from_rows(vec!["age".into()], rows), fp()).unwrap();
        assert_eq!(col(&d, 1).column_type, ColumnType::Integer);
        assert_eq!(col(&d, 1).distinct, Some(60));
        assert_eq!(col(&d, 1).level_bucket, None);
        assert!(col(&d, 1).levels.is_none());
        assert!(!col(&d, 1).is_group());
    }

    #[test]
    fn a_declared_string_variable_stays_text_even_when_its_cells_are_digits() {
        let mut raw = table(&["id"], &[&["001"], &["002"]]);
        raw.variables = vec![RawVariable { declared_text: Some(true), ..Default::default() }];
        let d = build_dataset(raw, fp()).unwrap();
        assert_eq!(col(&d, 1).column_type, ColumnType::Categorical);
        assert_eq!(col(&d, 1).levels.as_ref().unwrap().codes().collect::<Vec<_>>(), vec!["001", "002"]);
    }

    #[test]
    fn level_codes_unify_one_and_one_point_zero() {
        let d = build_dataset(table(&["g"], &[&["1"], &["1.0"], &["2"], &["2.00"]]), fp()).unwrap();
        let lv = col(&d, 1).levels.as_ref().unwrap();
        assert_eq!(lv.levels.iter().map(|l| (l.code.as_str(), l.count)).collect::<Vec<_>>(), vec![("1", 2), ("2", 2)]);
    }
}
