//! **The structural equation models' exports: the result as CSV bytes, the
//! manifest as JSON bytes (Research OS, Phase 9C-4, decision 800).** Bytes
//! only: the user's save dialog and the frontend's existing save path write
//! them, to the one file the user names (decision 5, ADR-004a). Nothing here
//! touches a file.
//!
//! The CSV is long, one row per number, in the eight fields every method's export
//! has, as the confirmatory factor analysis's: the rows used, the iterations
//! and the attempt; each loading and each path (its estimate, standard error,
//! C.R., P and standardized weight); each covariance with its correlation; each
//! variance and each error variance; each squared multiple correlation; each
//! effect, total, direct and indirect, standardized and not; every measure of
//! fit in its three rows; then the rows left out, the notes, the limitations and
//! the manifest's hashes.
//!
//! ```text
//! "table","statistic","column_index","column","column_index_2","column_2","value","word"
//! "loadings","estimate","1","C3","1","",0.61,""
//! "paths","estimate","2","","1","",0.37,""
//! "effects","indirect","3","","1","",0.12,""
//! "fit","rmsea","","","","",0.04,""
//! ```
//!
//! A line's first pair of fields is its first end: an arrow's `to`, a
//! covariance's `a`, a variance's variable; the second pair its second end: an
//! arrow's `from`, a covariance's `b`. A column is its index in the model's
//! columns, 0 to p − 1 in alias order, and its alias, never a header; a factor
//! is its number from 1 in the index field, its name field empty. A value that
//! is not defined is an empty field with its closed code in `word`. Each row
//! left out is two lines: its count, and the column it was found in, by its
//! alias's number (C2 is 2).
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule.

use super::{SemCode, SemOutcome, SemVariable, SemWeightKind, Stat};
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

/// A line's two ends, each a variable or nothing.
type Ends = (Option<SemVariable>, Option<SemVariable>);

fn end(columns: &[Alias], v: Option<SemVariable>) -> (String, String) {
    match v {
        None => (String::new(), String::new()),
        Some(SemVariable::Factor(n)) => (n.to_string(), String::new()),
        Some(SemVariable::Column(a)) => (columns.iter().position(|x| *x == a).unwrap_or(0).to_string(), a.0.name()),
    }
}

fn line(out: &mut String, columns: &[Alias], table: &str, statistic: &str, at: Ends, value: &str, word: &str) {
    let (i1, a1) = end(columns, at.0);
    let (i2, a2) = end(columns, at.1);
    let fields = [text(table), text(statistic), text(&i1), text(&a1), text(&i2), text(&a2), value.to_string(), text(word)];
    out.push_str(&fields.join(","));
    out.push_str("\r\n");
}

fn whole(out: &mut String, table: &str, statistic: &str, v: u64) {
    line(out, &[], table, statistic, (None, None), &v.to_string(), "");
}

fn stat(out: &mut String, columns: &[Alias], table: &str, statistic: &str, at: Ends, s: Stat) {
    match s {
        Stat::Value(v) => line(out, columns, table, statistic, at, &format!("{v}"), ""),
        Stat::NotComputable(c) => line(out, columns, table, statistic, at, "", c.wire()),
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark), closed by `manifest`'s
/// hashes: the manifest of the run that produced `result`.
pub fn result_csv(result: &SemOutcome, manifest: &RunManifest<SemCode>) -> Vec<u8> {
    let c = &result.columns;
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    whole(&mut out, "rows", "used", result.rows_used);
    whole(&mut out, "estimation", "iterations", result.iterations);
    whole(&mut out, "estimation", "attempt", u64::from(result.attempt));
    for w in &result.weights {
        let table = match w.kind {
            SemWeightKind::Loading => "loadings",
            SemWeightKind::Path => "paths",
        };
        let at = (Some(w.to), Some(w.from));
        stat(&mut out, c, table, "estimate", at, w.estimate);
        stat(&mut out, c, table, "se", at, w.se);
        stat(&mut out, c, table, "cr", at, w.cr);
        stat(&mut out, c, table, "p", at, w.p);
        stat(&mut out, c, table, "standardized", at, w.standardized);
    }
    for v in &result.covariances {
        let at = (Some(v.a), Some(v.b));
        stat(&mut out, c, "covariances", "estimate", at, v.estimate);
        stat(&mut out, c, "covariances", "se", at, v.se);
        stat(&mut out, c, "covariances", "cr", at, v.cr);
        stat(&mut out, c, "covariances", "p", at, v.p);
        stat(&mut out, c, "covariances", "correlation", at, v.correlation);
    }
    for v in &result.variances {
        let table = if v.error { "error_variances" } else { "variances" };
        let at = (Some(v.variable), None);
        stat(&mut out, c, table, "estimate", at, v.estimate);
        stat(&mut out, c, table, "se", at, v.se);
        stat(&mut out, c, table, "cr", at, v.cr);
        stat(&mut out, c, table, "p", at, v.p);
    }
    for s in &result.smc {
        stat(&mut out, c, "smc", "value", (Some(s.variable), None), s.value);
    }
    for e in &result.effects {
        let at = (Some(e.to), Some(e.from));
        stat(&mut out, c, "effects", "total", at, e.total);
        stat(&mut out, c, "effects", "direct", at, e.direct);
        stat(&mut out, c, "effects", "indirect", at, e.indirect);
        stat(&mut out, c, "effects", "total_standardized", at, e.total_standardized);
        stat(&mut out, c, "effects", "direct_standardized", at, e.direct_standardized);
        stat(&mut out, c, "effects", "indirect_standardized", at, e.indirect_standardized);
    }
    // The three rows of AMOS's tables (decision 796): the default model's as
    // table fit, then the saturated and independence models' with the same
    // statistic names.
    for (table, f) in [("fit", &result.fit.model), ("fit_saturated", &result.fit.saturated), ("fit_independence", &result.fit.independence)] {
        whole(&mut out, table, "npar", f.npar);
        stat(&mut out, c, table, "cmin", (None, None), f.cmin);
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
            stat(&mut out, c, table, name, (None, None), s);
        }
    }
    for e in &result.exclusions {
        line(&mut out, c, "excluded", "rows", (None, None), &e.count.to_string(), e.code.wire());
        let column = match e.params.get(&ParamKey::Column) {
            Some(ParamValue::Int(n)) => n.to_string(),
            _ => String::new(),
        };
        line(&mut out, c, "excluded", "column", (None, None), &column, e.code.wire());
    }
    for code in &result.notes {
        line(&mut out, c, "note", "", (None, None), "", code.wire());
    }
    for code in &result.limitations {
        line(&mut out, c, "limitation", "", (None, None), "", code.wire());
    }
    for h in &manifest.inputs {
        line(&mut out, c, "manifest", "input_sha256", (None, None), "", h.as_hex());
    }
    line(&mut out, c, "manifest", "parameters_sha256", (None, None), "", manifest.parameters_hash.as_hex());
    line(&mut out, c, "manifest", "output_sha256", (None, None), "", manifest.output_hash.as_hex());
    out.into_bytes()
}

/// The manifest as JSON bytes, indented for a reader.
pub fn manifest_json(manifest: &RunManifest<SemCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
