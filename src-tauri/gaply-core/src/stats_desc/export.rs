//! **The descriptive summary's exports: the result as CSV bytes, the
//! manifest as JSON bytes.** Bytes only: the user's save dialog and the
//! frontend's existing save path write them, to the one file the user names
//! (decision 5, ADR-004a). Nothing here touches a file.
//!
//! The CSV is long: one row per statistic, and one per level of a
//! categorical column.
//!
//! ```text
//! "alias","column","kind","statistic","level","value","note"
//! "C2","age","numeric","mean","",41.25,""
//! "C2","age","numeric","sd","",,"stats.desc.single_value"
//! "C3","group","categorical","count","b",12,""
//! ```
//!
//! * RFC 4180: CRLF line ends; every text field quoted, an inner `"`
//!   doubled. Numbers are not quoted, written in Rust's shortest form that
//!   reads back to the same double (`Display`); a statistic that is not
//!   computable has an empty value and its code in `note`.
//! * Spreadsheet formula injection: a text field that starts with `=`, `+`,
//!   `-`, `@`, a tab, a carriage return or an apostrophe gets one leading
//!   apostrophe. A reader strips exactly one leading apostrophe from a text
//!   field that has one, which gives back every field exactly (the reader
//!   lives in the round-trip test, not here).

use super::{ColumnResult, DescCode, DescribeResult, Stat};
use crate::contracts::codes::Closed;
use crate::contracts::RunManifest;

/// The first characters a spreadsheet may read as the start of a formula,
/// and the apostrophe itself, so that one guard apostrophe always marks one.
const FORMULA_STARTS: [char; 7] = ['=', '+', '-', '@', '\t', '\r', '\''];

const HEADER: [&str; 7] = [
    "alias",
    "column",
    "kind",
    "statistic",
    "level",
    "value",
    "note",
];

/// A text field, guarded and quoted.
fn text(field: &str) -> String {
    let guarded = if field.starts_with(FORMULA_STARTS) {
        format!("'{field}")
    } else {
        field.to_string()
    };
    format!("\"{}\"", guarded.replace('"', "\"\""))
}

fn row(
    out: &mut String,
    alias: &str,
    column: &str,
    kind: &str,
    statistic: &str,
    level: &str,
    value: &str,
    note: &str,
) {
    let fields = [
        text(alias),
        text(column),
        text(kind),
        text(statistic),
        text(level),
        value.to_string(),
        text(note),
    ];
    out.push_str(&fields.join(","));
    out.push_str("\r\n");
}

fn stat_fields(s: Stat) -> (String, &'static str) {
    match s {
        Stat::Value(v) => (format!("{v}"), ""),
        Stat::NotComputable(code) => (String::new(), <DescCode as Closed>::wire(code)),
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark).
pub fn result_csv(result: &DescribeResult) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    for column in &result.columns {
        match column {
            ColumnResult::Numeric(s) => {
                let alias = s.alias.0.name();
                row(
                    &mut out,
                    &alias,
                    &s.header,
                    "numeric",
                    "n",
                    "",
                    &s.n.to_string(),
                    "",
                );
                row(
                    &mut out,
                    &alias,
                    &s.header,
                    "numeric",
                    "missing",
                    "",
                    &s.missing.to_string(),
                    "",
                );
                for (name, stat) in [
                    ("mean", s.mean),
                    ("sd", s.sd),
                    ("min", s.min),
                    ("q1", s.q1),
                    ("median", s.median),
                    ("q3", s.q3),
                    ("max", s.max),
                ] {
                    let (value, note) = stat_fields(stat);
                    row(
                        &mut out, &alias, &s.header, "numeric", name, "", &value, note,
                    );
                }
            }
            ColumnResult::Categorical(s) => {
                let alias = s.alias.0.name();
                row(
                    &mut out,
                    &alias,
                    &s.header,
                    "categorical",
                    "n",
                    "",
                    &s.n.to_string(),
                    "",
                );
                row(
                    &mut out,
                    &alias,
                    &s.header,
                    "categorical",
                    "missing",
                    "",
                    &s.missing.to_string(),
                    "",
                );
                for level in &s.levels {
                    row(
                        &mut out,
                        &alias,
                        &s.header,
                        "categorical",
                        "count",
                        &level.label,
                        &level.count.to_string(),
                        "",
                    );
                }
            }
        }
    }
    out.into_bytes()
}

/// The manifest as JSON bytes, indented for a reader.
pub fn manifest_json(manifest: &RunManifest<DescCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
