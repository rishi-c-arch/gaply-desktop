//! **The t-test's exports: the result as CSV bytes, the manifest as JSON
//! bytes.** Bytes only: the user's save dialog and the frontend's existing
//! save path write them, to the one file the user names (decision 5,
//! ADR-004a). Nothing here touches a file.
//!
//! The CSV is long: one row per statistic, then each group's rows, the rows
//! left out and the limitations.
//!
//! ```text
//! "statistic","group","level","value","word"
//! "variant","","",,"welch"
//! "t","","",-2.5,""
//! "cohens_d","","",-0.71,"root_mean_variance"
//! "n","1","ctrl",12,""
//! "excluded","","",3,"stats.ttest.missing_outcome"
//! "limitation","","",,"stats.ttest.normality_not_assessed"
//! ```
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule: RFC
//! 4180 with CRLF line ends; every text field quoted, an inner `"` doubled;
//! numbers unquoted in Rust's shortest form that reads back to the same
//! double; a text field that starts with `=`, `+`, `-`, `@`, a tab, a carriage
//! return or an apostrophe gets one leading apostrophe. A value that is not
//! defined is an empty field with its code in `word`.

use super::{Standardizer, Stat, TTestCode, TTestResult, Variant};
use crate::contracts::codes::Closed;
use crate::contracts::RunManifest;
use crate::stats_desc::export::text;

const HEADER: [&str; 5] = ["statistic", "group", "level", "value", "word"];

fn row(out: &mut String, statistic: &str, group: &str, level: &str, value: &str, word: &str) {
    let fields = [
        text(statistic),
        text(group),
        text(level),
        value.to_string(),
        text(word),
    ];
    out.push_str(&fields.join(","));
    out.push_str("\r\n");
}

fn number(out: &mut String, statistic: &str, v: f64, word: &str) {
    row(out, statistic, "", "", &format!("{v}"), word);
}

fn variant_word(v: Variant) -> &'static str {
    match v {
        Variant::Student => "student",
        Variant::Welch => "welch",
    }
}

fn standardizer_word(s: Standardizer) -> &'static str {
    match s {
        Standardizer::PooledSd => "pooled_sd",
        Standardizer::RootMeanVariance => "root_mean_variance",
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark).
pub fn result_csv(result: &TTestResult) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    row(
        &mut out,
        "variant",
        "",
        "",
        "",
        variant_word(result.variant),
    );
    for (name, v) in [
        ("t", result.t),
        ("df", result.df),
        ("p", result.p),
        ("mean_difference", result.mean_difference),
        ("standard_error", result.standard_error),
        ("confidence_level", result.confidence_level),
        ("ci_lower", result.ci_lower),
        ("ci_upper", result.ci_upper),
    ] {
        number(&mut out, name, v, "");
    }
    let s = standardizer_word(result.d_standardizer);
    number(&mut out, "cohens_d", result.cohens_d, s);
    number(&mut out, "d_standardizer", result.d_standardizer_value, s);
    match result.sd_ratio {
        Stat::Value(v) => number(&mut out, "sd_ratio", v, ""),
        Stat::NotComputable(code) => row(&mut out, "sd_ratio", "", "", "", code.wire()),
    }
    for (k, g) in result.groups.iter().enumerate() {
        let group = (k + 1).to_string();
        for (name, value) in [
            ("level_index", g.level.to_string()),
            ("n", g.n.to_string()),
            ("mean", format!("{}", g.mean)),
            ("sd", format!("{}", g.sd)),
        ] {
            row(&mut out, name, &group, &g.label, &value, "");
        }
    }
    for e in &result.exclusions {
        row(
            &mut out,
            "excluded",
            "",
            "",
            &e.count.to_string(),
            e.code.wire(),
        );
    }
    for code in &result.limitations {
        row(
            &mut out,
            "limitation",
            "",
            "",
            "",
            <TTestCode as Closed>::wire(*code),
        );
    }
    out.into_bytes()
}

/// The manifest as JSON bytes, indented for a reader.
pub fn manifest_json(manifest: &RunManifest<TTestCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
