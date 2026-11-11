//! **The factor analysis's exports: the result as CSV bytes, the manifest as
//! JSON bytes (Research OS, Phase 9B-1, decision 701).** Bytes only: the user's
//! save dialog and the frontend's existing save path write them, to the one file
//! the user names (decision 5, ADR-004a). Nothing here touches a file.
//!
//! The CSV is long, one row per number, in the eight fields every method's export
//! has: each item's mean and standard deviation; the rows used; the determinant,
//! KMO with each item's sampling adequacy, Bartlett's test; every eigenvalue; the
//! extraction (its method, factors, iterations, communalities, sums and
//! loadings); the rotation, when there is one; then the rows left out, the
//! notes, the limitations and the manifest's hashes.
//!
//! ```text
//! "table","statistic","column_index","column","column_index_2","column_2","value","word"
//! "items","mean","0","C2","","",3.25,""
//! "eigenvalues","total","","","1","",2.4,""
//! "extraction_loadings","loading","0","C2","1","",0.81,""
//! "factor_correlations","phi","1","","2","",0.31,""
//! ```
//!
//! An item is its index in the spec's list, 0 to p − 1, and its alias, never a
//! header. A factor, an eigenvalue or a component is its number from 1 in the
//! second column fields' index, its name field empty; a line of a factor by a
//! factor (the transformation, the factor correlations) gives the first factor's
//! number in the first index field. A value that is not defined is an empty field
//! with its closed code in `word`. Each row left out is two lines: its count, and
//! the item it was found in, by its alias's number (C2 is 2).
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule.

use super::{Extraction, FactorCode, FactorOutcome, FactorSums, Rotation, Stat};
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

/// Where a line's number sits: an item (index and alias), a factor's number, or
/// both, or a factor by a factor.
#[derive(Clone, Copy)]
enum At {
    Nowhere,
    Item(usize, Alias),
    Number(usize),
    ItemByFactor(usize, Alias, usize),
    FactorByFactor(usize, usize),
}

fn line(out: &mut String, table: &str, statistic: &str, at: At, value: &str, word: &str) {
    let (i1, a1, i2) = match at {
        At::Nowhere => (String::new(), String::new(), String::new()),
        At::Item(i, a) => (i.to_string(), a.0.name(), String::new()),
        At::Number(k) => (String::new(), String::new(), k.to_string()),
        At::ItemByFactor(i, a, k) => (i.to_string(), a.0.name(), k.to_string()),
        At::FactorByFactor(j, k) => (j.to_string(), String::new(), k.to_string()),
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

fn sums(out: &mut String, table: &str, list: &[FactorSums]) {
    for (k, s) in list.iter().enumerate() {
        stat(out, table, "total", At::Number(k + 1), s.total);
        stat(out, table, "percent", At::Number(k + 1), s.percent);
        stat(out, table, "cumulative", At::Number(k + 1), s.cumulative);
    }
}

fn by_item(out: &mut String, table: &str, statistic: &str, items: &[Alias], m: &[Vec<Stat>]) {
    for (i, row) in m.iter().enumerate() {
        for (k, s) in row.iter().enumerate() {
            stat(out, table, statistic, At::ItemByFactor(i, items[i], k + 1), *s);
        }
    }
}

fn by_factor(out: &mut String, table: &str, statistic: &str, m: &[Vec<Stat>]) {
    for (j, row) in m.iter().enumerate() {
        for (k, s) in row.iter().enumerate() {
            stat(out, table, statistic, At::FactorByFactor(j + 1, k + 1), *s);
        }
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark), closed by `manifest`'s
/// hashes: the manifest of the run that produced `result`.
pub fn result_csv(result: &FactorOutcome, manifest: &RunManifest<FactorCode>) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    let items = &result.items;
    for (i, d) in result.descriptives.iter().enumerate() {
        stat(&mut out, "items", "mean", At::Item(i, d.item), d.mean);
        stat(&mut out, "items", "sd", At::Item(i, d.item), d.sd);
    }
    whole(&mut out, "rows", "used", result.rows_used);
    stat(&mut out, "determinant", "value", At::Nowhere, result.determinant.value);
    stat(&mut out, "determinant", "log", At::Nowhere, result.determinant.log);
    stat(&mut out, "kmo", "overall", At::Nowhere, result.kmo.overall);
    for (i, a) in result.kmo.items.iter().enumerate() {
        stat(&mut out, "kmo", "msa", At::Item(i, a.item), a.msa);
    }
    stat(&mut out, "bartlett", "chi_square", At::Nowhere, result.bartlett.chi_square);
    whole(&mut out, "bartlett", "df", result.bartlett.df);
    stat(&mut out, "bartlett", "p", At::Nowhere, result.bartlett.p);
    sums(&mut out, "eigenvalues", &result.eigenvalues);
    let e = &result.extraction;
    let method = match e.method {
        Extraction::PrincipalComponents => "principal_components",
        Extraction::PrincipalAxis => "principal_axis",
    };
    line(&mut out, "extraction", "method", At::Nowhere, "", method);
    whole(&mut out, "extraction", "factors", e.factors);
    whole(&mut out, "extraction", "iterations", e.iterations);
    for (i, c) in e.communalities.iter().enumerate() {
        stat(&mut out, "communalities", "initial", At::Item(i, c.item), c.initial);
        stat(&mut out, "communalities", "extraction", At::Item(i, c.item), c.extraction);
    }
    sums(&mut out, "extraction_sums", &e.sums);
    by_item(&mut out, "extraction_loadings", "loading", items, &e.loadings);
    if let Some(r) = &result.rotation {
        let method = match r.method {
            Rotation::None => "none",
            Rotation::Varimax => "varimax",
            Rotation::Promax => "promax",
        };
        line(&mut out, "rotation", "method", At::Nowhere, "", method);
        whole(&mut out, "rotation", "iterations", r.iterations);
        by_item(&mut out, "rotation_loadings", if r.method == Rotation::Promax { "pattern" } else { "loading" }, items, &r.loadings);
        if let Some(t) = &r.transformation {
            by_factor(&mut out, "rotation_transformation", "t", t);
        }
        if let Some(s) = &r.sums {
            sums(&mut out, "rotation_sums", s);
        }
        if let Some(s) = &r.structure {
            by_item(&mut out, "rotation_structure", "structure", items, s);
        }
        if let Some(phi) = &r.factor_correlations {
            by_factor(&mut out, "factor_correlations", "phi", phi);
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
pub fn manifest_json(manifest: &RunManifest<FactorCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
