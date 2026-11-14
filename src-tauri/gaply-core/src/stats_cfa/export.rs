//! **The confirmatory factor analysis's exports: the result as CSV bytes, the
//! manifest as JSON bytes (Research OS, Phase 9C-1, decision 746).** Bytes
//! only: the user's save dialog and the frontend's existing save path write
//! them, to the one file the user names (decision 5, ADR-004a). Nothing here
//! touches a file.
//!
//! The CSV is long, one row per number, in the eight fields every method's export
//! has, as the factor analysis's: the rows used and the iterations; each loading
//! (its estimate, standard error, C.R., P and standardized weight); each factor
//! variance; each factor covariance with its correlation; each error variance;
//! each squared multiple correlation; every measure of fit; then the rows left
//! out, the notes, the limitations and the manifest's hashes.
//!
//! ```text
//! "table","statistic","column_index","column","column_index_2","column_2","value","word"
//! "loadings","estimate","1","C3","1","",0.61,""
//! "factor_covariances","correlation","1","","2","",0.49,""
//! "fit","rmsea","","","","",0.04,""
//! ```
//!
//! A column is its index in the spec's list, 0 to p − 1, and its alias, never a
//! header. A factor is its number from 1 in the second index field, its name
//! field empty; a covariance gives its first factor's number in the first index
//! field. A value that is not defined is an empty field with its closed code in
//! `word`. Each row left out is two lines: its count, and the column it was
//! found in, by its alias's number (C2 is 2).
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule.

use super::{CfaCode, CfaOutcome, Stat};
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

/// Where a line's number sits: a column (index and alias), a factor's number,
/// a column on its factor, or a factor with a factor.
#[derive(Clone, Copy)]
enum At {
    Nowhere,
    Item(usize, Alias),
    Factor(u32),
    ItemOnFactor(usize, Alias, u32),
    FactorWithFactor(u32, u32),
}

fn line(out: &mut String, table: &str, statistic: &str, at: At, value: &str, word: &str) {
    let (i1, a1, i2) = match at {
        At::Nowhere => (String::new(), String::new(), String::new()),
        At::Item(i, a) => (i.to_string(), a.0.name(), String::new()),
        At::Factor(k) => (String::new(), String::new(), k.to_string()),
        At::ItemOnFactor(i, a, k) => (i.to_string(), a.0.name(), k.to_string()),
        At::FactorWithFactor(j, k) => (j.to_string(), String::new(), k.to_string()),
    };
    let fields = [text(table), text(statistic), text(&i1), text(&a1), text(&i2), text(""), value.to_string(), text(word)];
    out.push_str(&fields.join(","));
    out.push_str("\r\n");
}

fn whole(out: &mut String, table: &str, statistic: &str, v: u64) {
    line(out, table, statistic, At::Nowhere, &v.to_string(), "");
}

fn stat(out: &mut String, table: &str, statistic: &str, at: At, s: Stat) {
    match s {
        Stat::Value(v) => line(out, table, statistic, at, &format!("{v}"), ""),
        Stat::NotComputable(c) => line(out, table, statistic, at, "", c.wire()),
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark), closed by `manifest`'s
/// hashes: the manifest of the run that produced `result`.
pub fn result_csv(result: &CfaOutcome, manifest: &RunManifest<CfaCode>) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    whole(&mut out, "rows", "used", result.rows_used);
    whole(&mut out, "estimation", "iterations", result.iterations);
    let index = |a: Alias| result.items.iter().position(|x| *x == a).unwrap_or(0);
    for l in &result.loadings {
        let at = At::ItemOnFactor(index(l.item), l.item, l.factor);
        stat(&mut out, "loadings", "estimate", at, l.estimate);
        stat(&mut out, "loadings", "se", at, l.se);
        stat(&mut out, "loadings", "cr", at, l.cr);
        stat(&mut out, "loadings", "p", at, l.p);
        stat(&mut out, "loadings", "standardized", at, l.standardized);
    }
    for v in &result.factor_variances {
        let at = At::Factor(v.factor);
        stat(&mut out, "factor_variances", "estimate", at, v.estimate);
        stat(&mut out, "factor_variances", "se", at, v.se);
        stat(&mut out, "factor_variances", "cr", at, v.cr);
        stat(&mut out, "factor_variances", "p", at, v.p);
    }
    for c in &result.factor_covariances {
        let at = At::FactorWithFactor(c.factors[0], c.factors[1]);
        stat(&mut out, "factor_covariances", "estimate", at, c.estimate);
        stat(&mut out, "factor_covariances", "se", at, c.se);
        stat(&mut out, "factor_covariances", "cr", at, c.cr);
        stat(&mut out, "factor_covariances", "p", at, c.p);
        stat(&mut out, "factor_covariances", "correlation", at, c.correlation);
    }
    for e in &result.error_variances {
        let at = At::Item(index(e.item), e.item);
        stat(&mut out, "error_variances", "estimate", at, e.estimate);
        stat(&mut out, "error_variances", "se", at, e.se);
        stat(&mut out, "error_variances", "cr", at, e.cr);
        stat(&mut out, "error_variances", "p", at, e.p);
    }
    for s in &result.smc {
        stat(&mut out, "smc", "value", At::Item(index(s.item), s.item), s.value);
    }
    // The three rows of AMOS's tables (decision 763): the default model's as
    // table fit, then the saturated and independence models' with the same
    // statistic names.
    for (table, f) in [("fit", &result.fit.model), ("fit_saturated", &result.fit.saturated), ("fit_independence", &result.fit.independence)] {
        whole(&mut out, table, "npar", f.npar);
        stat(&mut out, table, "cmin", At::Nowhere, f.cmin);
        whole(&mut out, table, "df", f.df);
        for (name, s) in [
            ("p", f.p),
            ("cmin_df", f.cmin_df),
            ("rmr", f.rmr),
            ("gfi", f.gfi),
            ("agfi", f.agfi),
            ("pgfi", f.pgfi),
            ("nfi", f.nfi),
            ("rfi", f.rfi),
            ("ifi", f.ifi),
            ("tli", f.tli),
            ("cfi", f.cfi),
            ("pratio", f.pratio),
            ("pnfi", f.pnfi),
            ("pcfi", f.pcfi),
            ("ncp", f.ncp),
            ("ncp_lo90", f.ncp_lo90),
            ("ncp_hi90", f.ncp_hi90),
            ("fmin", f.fmin),
            ("f0", f.f0),
            ("f0_lo90", f.f0_lo90),
            ("f0_hi90", f.f0_hi90),
            ("rmsea", f.rmsea),
            ("rmsea_lo90", f.rmsea_lo90),
            ("rmsea_hi90", f.rmsea_hi90),
            ("pclose", f.pclose),
            ("aic", f.aic),
            ("bcc", f.bcc),
            ("bic", f.bic),
            ("caic", f.caic),
            ("ecvi", f.ecvi),
            ("ecvi_lo90", f.ecvi_lo90),
            ("ecvi_hi90", f.ecvi_hi90),
            ("mecvi", f.mecvi),
            ("hoelter_05", f.hoelter05),
            ("hoelter_01", f.hoelter01),
        ] {
            stat(&mut out, table, name, At::Nowhere, s);
        }
    }
    for e in &result.exclusions {
        line(&mut out, "excluded", "rows", At::Nowhere, &e.count.to_string(), e.code.wire());
        let column = match e.params.get(&ParamKey::Column) {
            Some(ParamValue::Int(n)) => n.to_string(),
            _ => String::new(),
        };
        line(&mut out, "excluded", "column", At::Nowhere, &column, e.code.wire());
    }
    for c in &result.notes {
        line(&mut out, "note", "", At::Nowhere, "", c.wire());
    }
    for c in &result.limitations {
        line(&mut out, "limitation", "", At::Nowhere, "", c.wire());
    }
    for h in &manifest.inputs {
        line(&mut out, "manifest", "input_sha256", At::Nowhere, "", h.as_hex());
    }
    line(&mut out, "manifest", "parameters_sha256", At::Nowhere, "", manifest.parameters_hash.as_hex());
    line(&mut out, "manifest", "output_sha256", At::Nowhere, "", manifest.output_hash.as_hex());
    out.into_bytes()
}

/// The manifest as JSON bytes, indented for a reader.
pub fn manifest_json(manifest: &RunManifest<CfaCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
