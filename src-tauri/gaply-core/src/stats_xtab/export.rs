//! **Crosstabs' exports: the result as CSV bytes, the manifest as JSON
//! bytes (Research OS, Phase 8X-1, decision 577).** Bytes only: the user's
//! save dialog and the frontend's existing save path write them, to the one
//! file the user names (decision 5, ADR-004a). Nothing here touches a file.
//!
//! The CSV is long, one row per number: each cell's numbers, row by row;
//! the row totals, the column totals, n and the expected counts' facts;
//! Pearson's chi-square, the likelihood ratio, then for a 2 x 2 table the
//! continuity-corrected chi-square and Fisher's exact test; phi, Cramér's V
//! and the contingency coefficient; then the rows left out, the notes, the
//! limitations and the manifest's hashes.
//!
//! ```text
//! "table","statistic","row_index","row_level","column_index","column_level","value","word"
//! "cells","observed","0","no","1","yes",12,""
//! "cells","expected","0","no","1","yes",9.5,""
//! "row_totals","n","0","no","","",30,""
//! "pearson","p","","","","",0.031,""
//! "measures","phi","","","","",,"stats.xtab.arithmetic_refused"
//! "excluded","rows","","","","",3,"stats.xtab.missing_code"
//! "excluded","column","","","","",2,"stats.xtab.missing_code"
//! "manifest","output_sha256","","","","",,"9f…"
//! ```
//!
//! A level is its index in its column's code-point order and its label. A
//! value that is not defined is an empty field with its closed code in
//! `word`. Each row left out is two lines: its count, and the column it was
//! found in, by the position the spec names it by.
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule: RFC
//! 4180 with CRLF line ends; every text field quoted, an inner `"` doubled;
//! numbers unquoted in Rust's shortest form that reads back to the same
//! double; a text field that starts with `=`, `+`, `-`, `@`, a tab, a carriage
//! return or an apostrophe gets one leading apostrophe.

use super::{ChiOutcome, CrosstabsOutcome, Stat, XtabCode};
use crate::contracts::codes::Closed;
use crate::contracts::{ParamKey, ParamValue, RunManifest};
use crate::stats_desc::export::text;

const HEADER: [&str; 8] = [
    "table",
    "statistic",
    "row_index",
    "row_level",
    "column_index",
    "column_level",
    "value",
    "word",
];

/// A line's two levels: each an index and its label, or empty.
struct At<'a> {
    row: Option<(u32, &'a str)>,
    column: Option<(u32, &'a str)>,
}

const NOWHERE: At<'static> = At {
    row: None,
    column: None,
};

fn line(out: &mut String, table: &str, statistic: &str, at: &At, value: &str, word: &str) {
    let level = |l: Option<(u32, &str)>| match l {
        Some((i, label)) => (text(&i.to_string()), text(label)),
        None => (text(""), text("")),
    };
    let (i1, l1) = level(at.row);
    let (i2, l2) = level(at.column);
    let fields = [
        text(table),
        text(statistic),
        i1,
        l1,
        i2,
        l2,
        value.to_string(),
        text(word),
    ];
    out.push_str(&fields.join(","));
    out.push_str("\r\n");
}

fn number(out: &mut String, table: &str, statistic: &str, at: &At, v: f64) {
    line(out, table, statistic, at, &format!("{v}"), "");
}

fn whole(out: &mut String, table: &str, statistic: &str, at: &At, v: u64) {
    line(out, table, statistic, at, &v.to_string(), "");
}

fn code(out: &mut String, table: &str, statistic: &str, c: XtabCode) {
    line(out, table, statistic, &NOWHERE, "", c.wire());
}

fn stat(out: &mut String, table: &str, statistic: &str, s: Stat) {
    match s {
        Stat::Value(v) => number(out, table, statistic, &NOWHERE, v),
        Stat::NotComputable(c) => code(out, table, statistic, c),
    }
}

fn chi(out: &mut String, table: &str, t: ChiOutcome) {
    match t {
        ChiOutcome::Computed(t) => {
            number(out, table, "statistic", &NOWHERE, t.statistic);
            number(out, table, "df", &NOWHERE, t.df);
            number(out, table, "p", &NOWHERE, t.p);
        }
        ChiOutcome::NotComputable(c) => code(out, table, "not_computable", c),
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark), closed by
/// `manifest`'s hashes: the manifest of the run that produced `result`.
pub fn result_csv(result: &CrosstabsOutcome, manifest: &RunManifest<XtabCode>) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    let row_label = |i: u32| result.row_levels[i as usize].label.as_str();
    let column_label = |j: u32| result.column_levels[j as usize].label.as_str();
    for f in &result.frequencies {
        let at = At {
            row: Some((f.row, row_label(f.row))),
            column: Some((f.column, column_label(f.column))),
        };
        whole(&mut out, "cells", "observed", &at, f.observed);
        for (name, v) in [
            ("expected", f.expected),
            ("row_proportion", f.row_proportion),
            ("column_proportion", f.column_proportion),
            ("total_proportion", f.total_proportion),
            ("residual", f.residual),
            ("pearson_residual", f.pearson_residual),
            ("adjusted_residual", f.adjusted_residual),
        ] {
            number(&mut out, "cells", name, &at, v);
        }
    }
    for (i, n) in result.row_totals.iter().enumerate() {
        let at = At {
            row: Some((i as u32, row_label(i as u32))),
            column: None,
        };
        whole(&mut out, "row_totals", "n", &at, *n);
    }
    for (j, n) in result.column_totals.iter().enumerate() {
        let at = At {
            row: None,
            column: Some((j as u32, column_label(j as u32))),
        };
        whole(&mut out, "column_totals", "n", &at, *n);
    }
    whole(&mut out, "table", "n", &NOWHERE, result.n);
    whole(&mut out, "table", "cells", &NOWHERE, result.cell_count);
    whole(
        &mut out,
        "table",
        "expected_below_five",
        &NOWHERE,
        result.expected_below_five,
    );
    number(
        &mut out,
        "table",
        "smallest_expected",
        &NOWHERE,
        result.smallest_expected,
    );
    chi(&mut out, "pearson", result.pearson);
    chi(&mut out, "likelihood_ratio", result.likelihood_ratio);
    if let Some(t) = result.continuity_corrected {
        chi(&mut out, "continuity_corrected", t);
    }
    if let Some(f) = result.fisher {
        number(&mut out, "fisher", "two_sided", &NOWHERE, f.two_sided);
        number(&mut out, "fisher", "less", &NOWHERE, f.less);
        number(&mut out, "fisher", "greater", &NOWHERE, f.greater);
    }
    stat(&mut out, "measures", "phi", result.phi);
    stat(&mut out, "measures", "cramers_v", result.cramers_v);
    stat(
        &mut out,
        "measures",
        "contingency_coefficient",
        result.contingency_coefficient,
    );
    for e in &result.exclusions {
        line(
            &mut out,
            "excluded",
            "rows",
            &NOWHERE,
            &e.count.to_string(),
            e.code.wire(),
        );
        let column = match e.params.get(&ParamKey::Column) {
            Some(ParamValue::Int(n)) => n.to_string(),
            _ => String::new(),
        };
        line(&mut out, "excluded", "column", &NOWHERE, &column, e.code.wire());
    }
    for c in &result.notes {
        code(&mut out, "note", "", *c);
    }
    for c in &result.limitations {
        code(&mut out, "limitation", "", *c);
    }
    for h in &manifest.inputs {
        line(
            &mut out,
            "manifest",
            "input_sha256",
            &NOWHERE,
            "",
            h.as_hex(),
        );
    }
    line(
        &mut out,
        "manifest",
        "parameters_sha256",
        &NOWHERE,
        "",
        manifest.parameters_hash.as_hex(),
    );
    line(
        &mut out,
        "manifest",
        "output_sha256",
        &NOWHERE,
        "",
        manifest.output_hash.as_hex(),
    );
    out.into_bytes()
}

/// The manifest as JSON bytes, indented for a reader.
pub fn manifest_json(manifest: &RunManifest<XtabCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
