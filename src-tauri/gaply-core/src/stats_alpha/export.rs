//! **Cronbach's alpha's exports: the result as CSV bytes, the manifest as
//! JSON bytes (Research OS, Phase 8I-2, decision 379).** Bytes only: the
//! user's save dialog and the frontend's existing save path write them, to
//! the one file the user names (decision 5, ADR-004a). Nothing here touches
//! a file.
//!
//! The CSV is long, one row per number, in the order the screen shows it:
//! the confidence level, the items (k), the rows read and used; alpha, its
//! interval's ends, standardized alpha and the inter-item correlations' mean,
//! smallest and largest; then each item's block, in the items' order
//! (whether it was reversed, as 1 or 0, its mean and SD, the scale's mean and
//! variance if it were left out, its corrected item-total correlation and
//! alpha if it were left out); then the rows left out, the note, the
//! limitations and the manifest's hashes.
//!
//! ```text
//! "table","statistic","first_index","first","second_index","second","value","word"
//! "result","confidence_level","","","","",0.95,""
//! "alpha","alpha","","","","",0.84,""
//! "alpha","standardized","","","","",,"stats.alpha.item_constant"
//! "items","reversed","1","C3","","",1,""
//! "items","alpha_if_deleted","0","C2","","",,"stats.alpha.two_items"
//! "excluded","rows","0","C2","","",1,"stats.alpha.missing_code"
//! "note","","","","","",,"stats.alpha.negative_alpha"
//! "manifest","output_sha256","","","","",,"9f…"
//! ```
//!
//! **Columns are named by alias** (`C1`, `C2`, …), with their index among
//! the items, never by header: a stored result names them so, so the same
//! run, live or rerun from a project, exports the same bytes. The scale's
//! ends are a setting, not a result: the project's report gives them, the
//! CSV does not. A value that is not defined is an empty field with its
//! closed code in `word`.
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), as the correlations' export uses them: RFC
//! 4180 with CRLF line ends; every text field quoted, an inner `"` doubled;
//! numbers unquoted in Rust's shortest form that reads back to the same
//! double; a text field that starts with `=`, `+`, `-`, `@`, a tab, a carriage
//! return or an apostrophe gets one leading apostrophe.

use super::{AlphaCode, AlphaOutcome, Interval, Stat, CONFIDENCE_LEVEL};
use crate::contracts::codes::Closed;
use crate::contracts::{ParamKey, ParamValue, RunManifest};
use crate::stats_desc::export::text;

const HEADER: [&str; 8] = [
    "table",
    "statistic",
    "first_index",
    "first",
    "second_index",
    "second",
    "value",
    "word",
];

/// An item: its index among the items and its alias.
type At = Option<(usize, String)>;

fn row(out: &mut String, table: &str, statistic: &str, at: &At, value: &str, word: &str) {
    let (i, a) = match at {
        Some((i, alias)) => (text(&i.to_string()), text(alias)),
        None => (text(""), text("")),
    };
    let fields = [
        text(table),
        text(statistic),
        i,
        a,
        text(""),
        text(""),
        value.to_string(),
        text(word),
    ];
    out.push_str(&fields.join(","));
    out.push_str("\r\n");
}

fn number(out: &mut String, table: &str, statistic: &str, at: &At, v: f64) {
    row(out, table, statistic, at, &format!("{v}"), "");
}

fn code(out: &mut String, table: &str, statistic: &str, at: &At, c: AlphaCode) {
    row(out, table, statistic, at, "", c.wire());
}

fn stat(out: &mut String, table: &str, statistic: &str, at: &At, s: Stat) {
    match s {
        Stat::Value(v) => number(out, table, statistic, at, v),
        Stat::NotComputable(c) => code(out, table, statistic, at, c),
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark), closed by
/// `manifest`'s hashes: the manifest of the run that produced `result`.
pub fn result_csv(result: &AlphaOutcome, manifest: &RunManifest<AlphaCode>) -> Vec<u8> {
    let none: At = None;
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    number(
        &mut out,
        "result",
        "confidence_level",
        &none,
        CONFIDENCE_LEVEL,
    );
    for (name, count) in [
        ("k", result.k),
        ("rows_read", manifest.rows_read),
        ("rows_used", result.rows_used),
    ] {
        row(&mut out, "result", name, &none, &count.to_string(), "");
    }
    stat(&mut out, "alpha", "alpha", &none, result.alpha);
    match result.interval {
        Interval::Computed { lower, upper } => {
            number(&mut out, "alpha", "ci_lower", &none, lower);
            number(&mut out, "alpha", "ci_upper", &none, upper);
        }
        Interval::NotComputable(c) => {
            code(&mut out, "alpha", "ci_lower", &none, c);
            code(&mut out, "alpha", "ci_upper", &none, c);
        }
    }
    stat(
        &mut out,
        "alpha",
        "standardized",
        &none,
        result.standardized,
    );
    let r = result.inter_item;
    for (name, s) in [
        ("inter_item_mean", r.mean),
        ("inter_item_min", r.min),
        ("inter_item_max", r.max),
    ] {
        stat(&mut out, "alpha", name, &none, s);
    }
    for (i, item) in result.items.iter().enumerate() {
        let here: At = Some((i, item.item.0.name()));
        let reversed = if item.reversed { "1" } else { "0" };
        row(&mut out, "items", "reversed", &here, reversed, "");
        for (name, s) in [
            ("mean", item.mean),
            ("sd", item.sd),
            ("scale_mean_if_deleted", item.scale_mean_if_deleted),
            ("scale_variance_if_deleted", item.scale_variance_if_deleted),
            ("corrected_item_total", item.corrected_item_total),
            ("alpha_if_deleted", item.alpha_if_deleted),
        ] {
            stat(&mut out, "items", name, &here, s);
        }
    }
    for e in &result.exclusions {
        // Every exclusion names its column's alias number.
        let column: At = match e.params.get(&ParamKey::Column) {
            Some(ParamValue::Int(n)) => result
                .items
                .iter()
                .position(|it| i64::from(it.item.0 .0) == *n)
                .map(|i| (i, result.items[i].item.0.name())),
            _ => None,
        };
        row(
            &mut out,
            "excluded",
            "rows",
            &column,
            &e.count.to_string(),
            e.code.wire(),
        );
    }
    for c in &result.notes {
        code(&mut out, "note", "", &none, *c);
    }
    for c in &result.limitations {
        code(&mut out, "limitation", "", &none, *c);
    }
    for h in &manifest.inputs {
        row(&mut out, "manifest", "input_sha256", &none, "", h.as_hex());
    }
    row(
        &mut out,
        "manifest",
        "parameters_sha256",
        &none,
        "",
        manifest.parameters_hash.as_hex(),
    );
    row(
        &mut out,
        "manifest",
        "output_sha256",
        &none,
        "",
        manifest.output_hash.as_hex(),
    );
    out.into_bytes()
}

/// The manifest as JSON bytes, indented for a reader.
pub fn manifest_json(manifest: &RunManifest<AlphaCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
