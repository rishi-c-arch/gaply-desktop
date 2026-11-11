//! **The linear regression's exports: the result as CSV bytes, the manifest as
//! JSON bytes (Research OS, Phase 9A-1, decision 671).** Bytes only: the user's
//! save dialog and the frontend's existing save path write them, to the one file
//! the user names (decision 5, ADR-004a). Nothing here touches a file.
//!
//! The CSV is long, one row per number, in the eight fields every method's export
//! has: each model column's mean and standard deviation (the outcome first); the
//! rows used; the model summary; the analysis of variance; the intercept; each
//! predictor in the list's order; the residuals' summary; then the rows left out,
//! the notes, the limitations and the manifest's hashes.
//!
//! ```text
//! "table","statistic","column_index","column","column_index_2","column_2","value","word"
//! "columns","mean","0","C2","","",5.25,""
//! "rows","used","","","","",25,""
//! "model","r_squared","","","","",0.61,""
//! "anova","f","","","","",,"stats.linear_regression.perfect_fit"
//! "intercept","b","","","","",1.5,""
//! "predictor","b","1","C3","","",0.75,""
//! "excluded","rows","","","","",3,"stats.linear_regression.missing_code"
//! "excluded","column","","","","",2,"stats.linear_regression.missing_code"
//! "manifest","output_sha256","","","","",,"9f…"
//! ```
//!
//! A column is its index in the model's list, the outcome 0 and the predictors 1
//! to k in the user's order, and its alias, never a header; the related rank
//! tests' export numbers its columns by the list the user chose in the same way.
//! The intercept's lines leave both column fields empty: it is no column. A value
//! that is not defined is an empty field with its closed code in `word`. Each row
//! left out is two lines: its count, and the column it was found in, by its
//! alias's number (C2 is 2), as the related rank tests' export gives it.
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule.

use super::{LinearRegressionCode, LinearRegressionOutcome, Stat};
use crate::contracts::codes::Closed;
use crate::contracts::{ParamKey, ParamValue, RunManifest};
use crate::stats_desc::export::text;
use crate::stats_desc::Alias;

const HEADER: [&str; 8] = [
    "table",
    "statistic",
    "column_index",
    "column",
    "column_index_2",
    "column_2",
    "value",
    "word",
];

fn line(out: &mut String, table: &str, statistic: &str, at: Option<(usize, Alias)>, value: &str, word: &str) {
    let (i1, a1) = match at {
        Some((i, alias)) => (text(&i.to_string()), text(&alias.0.name())),
        None => (text(""), text("")),
    };
    let fields = [
        text(table),
        text(statistic),
        i1,
        a1,
        text(""),
        text(""),
        value.to_string(),
        text(word),
    ];
    out.push_str(&fields.join(","));
    out.push_str("\r\n");
}

fn whole(out: &mut String, table: &str, statistic: &str, at: Option<(usize, Alias)>, v: u64) {
    line(out, table, statistic, at, &v.to_string(), "");
}

fn stat(out: &mut String, table: &str, statistic: &str, at: Option<(usize, Alias)>, s: Stat) {
    match s {
        Stat::Value(v) => line(out, table, statistic, at, &format!("{v}"), ""),
        Stat::NotComputable(c) => line(out, table, statistic, at, "", c.wire()),
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark), closed by `manifest`'s
/// hashes: the manifest of the run that produced `result`.
pub fn result_csv(
    result: &LinearRegressionOutcome,
    manifest: &RunManifest<LinearRegressionCode>,
) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    for (i, c) in result.columns.iter().enumerate() {
        let at = Some((i, c.column));
        stat(&mut out, "columns", "mean", at, c.mean);
        stat(&mut out, "columns", "sd", at, c.sd);
    }
    whole(&mut out, "rows", "used", None, result.rows_used);
    let m = &result.model;
    for (name, s) in [
        ("r", m.r),
        ("r_squared", m.r_squared),
        ("adjusted_r_squared", m.adjusted_r_squared),
        ("standard_error_of_estimate", m.standard_error_of_estimate),
        ("durbin_watson", m.durbin_watson),
    ] {
        stat(&mut out, "model", name, None, s);
    }
    let a = &result.anova;
    for (source, s) in [("regression", &a.regression), ("residual", &a.residual)] {
        stat(&mut out, "anova", &format!("{source}_sum_of_squares"), None, s.sum_of_squares);
        whole(&mut out, "anova", &format!("{source}_df"), None, s.df);
        stat(&mut out, "anova", &format!("{source}_mean_square"), None, s.mean_square);
    }
    stat(&mut out, "anova", "total_sum_of_squares", None, a.total.sum_of_squares);
    whole(&mut out, "anova", "total_df", None, a.total.df);
    stat(&mut out, "anova", "f", None, a.f);
    stat(&mut out, "anova", "p", None, a.p);
    let c = &result.intercept;
    for (name, s) in [
        ("b", c.b),
        ("standard_error", c.standard_error),
        ("t", c.t),
        ("p", c.p),
        ("lower", c.lower),
        ("upper", c.upper),
    ] {
        stat(&mut out, "intercept", name, None, s);
    }
    for (j, p) in result.predictors.iter().enumerate() {
        let at = Some((j + 1, p.column));
        for (name, s) in [
            ("b", p.b),
            ("standard_error", p.standard_error),
            ("beta", p.beta),
            ("t", p.t),
            ("p", p.p),
            ("lower", p.lower),
            ("upper", p.upper),
            ("zero_order", p.zero_order),
            ("partial", p.partial),
            ("part", p.part),
            ("tolerance", p.tolerance),
            ("vif", p.vif),
        ] {
            stat(&mut out, "predictor", name, at, s);
        }
    }
    let r = &result.residuals;
    for (variable, s) in [("predicted", &r.predicted), ("residual", &r.residual)] {
        stat(&mut out, "residuals", &format!("{variable}_minimum"), None, s.minimum);
        stat(&mut out, "residuals", &format!("{variable}_maximum"), None, s.maximum);
        stat(&mut out, "residuals", &format!("{variable}_mean"), None, s.mean);
        stat(&mut out, "residuals", &format!("{variable}_standard_deviation"), None, s.standard_deviation);
    }
    for (variable, s) in [
        ("standardized_predicted", &r.standardized_predicted),
        ("standardized_residual", &r.standardized_residual),
    ] {
        stat(&mut out, "residuals", &format!("{variable}_minimum"), None, s.minimum);
        stat(&mut out, "residuals", &format!("{variable}_maximum"), None, s.maximum);
    }
    for e in &result.exclusions {
        line(&mut out, "excluded", "rows", None, &e.count.to_string(), e.code.wire());
        let column = match e.params.get(&ParamKey::Column) {
            Some(ParamValue::Int(n)) => n.to_string(),
            _ => String::new(),
        };
        line(&mut out, "excluded", "column", None, &column, e.code.wire());
    }
    for c in &result.notes {
        line(&mut out, "note", "", None, "", c.wire());
    }
    for c in &result.limitations {
        line(&mut out, "limitation", "", None, "", c.wire());
    }
    for h in &manifest.inputs {
        line(&mut out, "manifest", "input_sha256", None, "", h.as_hex());
    }
    line(&mut out, "manifest", "parameters_sha256", None, "", manifest.parameters_hash.as_hex());
    line(&mut out, "manifest", "output_sha256", None, "", manifest.output_hash.as_hex());
    out.into_bytes()
}

/// The manifest as JSON bytes, indented for a reader.
pub fn manifest_json(manifest: &RunManifest<LinearRegressionCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
