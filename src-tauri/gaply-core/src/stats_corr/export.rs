//! **The correlations' exports: the result as CSV bytes, the manifest as
//! JSON bytes (Research OS, Phase 8H-2, decision 354).** Bytes only: the
//! user's save dialog and the frontend's existing save path write them, to
//! the one file the user names (decision 5, ADR-004a). Nothing here touches
//! a file.
//!
//! The CSV is long, one row per number, in the order the screen shows it:
//! the confidence level, the rows read and used, the descriptives, then each
//! chosen coefficient's pairs (n, the coefficient, t or z, df where there is
//! one, p, the interval's ends); then the rows left out, the limitations and
//! the manifest's hashes.
//!
//! ```text
//! "table","statistic","first_index","first","second_index","second","value","word"
//! "result","confidence_level","","","","",0.95,""
//! "descriptives","mean","0","C1","","",2.5,""
//! "pearson","r","0","C1","1","C2",0.61,""
//! "pearson","t","0","C1","2","C3",,"stats.corr.perfect_coefficient"
//! "pearson","p","0","C1","1","C2",0.0123,""
//! "kendall","ci_lower","1","C2","2","C3",,"stats.corr.interval_too_few_rows"
//! "excluded","rows","1","C2","","",3,"stats.corr.missing_code"
//! "manifest","output_sha256","","","","",,"9f…"
//! ```
//!
//! **Columns are named by alias** (`C1`, `C2`, …), with their index among
//! the chosen variables, never by header: a stored result names them so, so
//! the same run, live or rerun from a project, exports the same bytes. A
//! value that is not defined is an empty field with its closed code in
//! `word`; an infinite t is the word `stats.corr.perfect_coefficient`, never
//! a number.
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule: RFC
//! 4180 with CRLF line ends; every text field quoted, an inner `"` doubled;
//! numbers unquoted in Rust's shortest form that reads back to the same
//! double; a text field that starts with `=`, `+`, `-`, `@`, a tab, a carriage
//! return or an apostrophe gets one leading apostrophe.

use super::{
    Alias, Bounded, Coefficient, CorrCode, CorrOutcome, Interval, PairOutcome, Stat, TestStatistic,
    CONFIDENCE_LEVEL,
};
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

/// A variable: its index among the chosen variables and its alias.
type At = Option<(usize, String)>;

fn row(out: &mut String, table: &str, statistic: &str, at: [&At; 2], value: &str, word: &str) {
    let side = |a: &At| match a {
        Some((i, alias)) => (text(&i.to_string()), text(alias)),
        None => (text(""), text("")),
    };
    let (i1, a1) = side(at[0]);
    let (i2, a2) = side(at[1]);
    let fields = [
        text(table),
        text(statistic),
        i1,
        a1,
        i2,
        a2,
        value.to_string(),
        text(word),
    ];
    out.push_str(&fields.join(","));
    out.push_str("\r\n");
}

fn number(out: &mut String, table: &str, statistic: &str, at: [&At; 2], v: f64) {
    row(out, table, statistic, at, &format!("{v}"), "");
}

fn code(out: &mut String, table: &str, statistic: &str, at: [&At; 2], c: CorrCode) {
    row(out, table, statistic, at, "", c.wire());
}

fn stat(out: &mut String, table: &str, statistic: &str, at: [&At; 2], s: Stat) {
    match s {
        Stat::Value(v) => number(out, table, statistic, at, v),
        Stat::NotComputable(c) => code(out, table, statistic, at, c),
    }
}

/// The coefficient's table name and its own statistic's.
fn names(c: Coefficient) -> (&'static str, &'static str) {
    match c {
        Coefficient::Pearson => ("pearson", "r"),
        Coefficient::Spearman => ("spearman", "rho"),
        Coefficient::Kendall => ("kendall", "tau_b"),
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark), closed by
/// `manifest`'s hashes: the manifest of the run that produced `result`.
pub fn result_csv(result: &CorrOutcome, manifest: &RunManifest<CorrCode>) -> Vec<u8> {
    let none: At = None;
    let nothing = [&none, &none];
    // The variables in the result's order, each by its index and alias.
    let aliases: Vec<_> = result.descriptives.iter().map(|d| d.variable).collect();
    let at = |alias: Alias| -> At {
        aliases
            .iter()
            .position(|a| *a == alias)
            .map(|i| (i, alias.0.name()))
    };
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    number(
        &mut out,
        "result",
        "confidence_level",
        nothing,
        CONFIDENCE_LEVEL,
    );
    for (name, count) in [
        ("rows_read", manifest.rows_read),
        ("rows_used", result.rows_used),
    ] {
        row(&mut out, "result", name, nothing, &count.to_string(), "");
    }
    for d in &result.descriptives {
        let first = at(d.variable);
        let here = [&first, &none];
        row(&mut out, "descriptives", "n", here, &d.n.to_string(), "");
        stat(&mut out, "descriptives", "mean", here, d.mean);
        stat(&mut out, "descriptives", "sd", here, d.sd);
    }
    for table in &result.coefficients {
        let (name, estimate) = names(table.coefficient);
        for p in &table.pairs {
            let (first, second) = (at(p.first), at(p.second));
            let here = [&first, &second];
            row(&mut out, name, "n", here, &p.n.to_string(), "");
            let s = match p.outcome {
                PairOutcome::Computed(s) => s,
                PairOutcome::NotComputable(c) => {
                    code(&mut out, name, "not_computable", here, c);
                    continue;
                }
            };
            number(&mut out, name, estimate, here, s.estimate);
            match s.test {
                TestStatistic::T(t) => {
                    match t.t {
                        Bounded::Value(v) => number(&mut out, name, "t", here, v),
                        Bounded::Infinite(c) => code(&mut out, name, "t", here, c),
                    }
                    number(&mut out, name, "df", here, t.df);
                }
                TestStatistic::Z(z) => number(&mut out, name, "z", here, z),
            }
            number(&mut out, name, "p", here, s.p);
            match s.interval {
                Interval::Computed { lower, upper } => {
                    number(&mut out, name, "ci_lower", here, lower);
                    number(&mut out, name, "ci_upper", here, upper);
                }
                Interval::NotComputable(c) => {
                    code(&mut out, name, "ci_lower", here, c);
                    code(&mut out, name, "ci_upper", here, c);
                }
            }
        }
    }
    for e in &result.exclusions {
        // A listwise exclusion names its column's alias number; a pairwise
        // one names none.
        let column: At = match e.params.get(&ParamKey::Column) {
            Some(ParamValue::Int(n)) => aliases
                .iter()
                .position(|a| i64::from(a.0 .0) == *n)
                .map(|i| (i, aliases[i].0.name())),
            _ => None,
        };
        row(
            &mut out,
            "excluded",
            "rows",
            [&column, &none],
            &e.count.to_string(),
            e.code.wire(),
        );
    }
    for c in &result.limitations {
        code(&mut out, "limitation", "", nothing, *c);
    }
    for h in &manifest.inputs {
        row(
            &mut out,
            "manifest",
            "input_sha256",
            nothing,
            "",
            h.as_hex(),
        );
    }
    row(
        &mut out,
        "manifest",
        "parameters_sha256",
        nothing,
        "",
        manifest.parameters_hash.as_hex(),
    );
    row(
        &mut out,
        "manifest",
        "output_sha256",
        nothing,
        "",
        manifest.output_hash.as_hex(),
    );
    out.into_bytes()
}

/// The manifest as JSON bytes, indented for a reader.
pub fn manifest_json(manifest: &RunManifest<CorrCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
