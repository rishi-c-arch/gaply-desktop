//! **The one-sample and paired t-tests' exports: each result as CSV bytes,
//! the manifest as JSON bytes.** Bytes only: the user's save dialog and the
//! frontend's existing save path write them, to the one file the user names
//! (decision 5, ADR-004a). Nothing here touches a file.
//!
//! The CSV is long: one row per statistic, then the rows left out and the
//! limitations. `column` is `first` or `second` on the paired test's column
//! rows, and empty otherwise.
//!
//! ```text
//! "statistic","column","value","word"
//! "n_pairs","",12,""
//! "mean","first",21.4,""
//! "t","",-1.77,""
//! "excluded","",1,"stats.onesample.missing_second"
//! "limitation","",,"stats.onesample.cohens_d_has_no_interval"
//! ```
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule: RFC
//! 4180 with CRLF line ends; every text field quoted, an inner `"` doubled;
//! numbers unquoted in Rust's shortest form that reads back to the same
//! double; a text field that starts with `=`, `+`, `-`, `@`, a tab, a carriage
//! return or an apostrophe gets one leading apostrophe.

use super::{OneSampleCode, OneSampleResult, PairedResult};
use crate::contracts::codes::Closed;
use crate::contracts::{Exclusion, RunManifest};
use crate::stats_desc::export::text;

const HEADER: [&str; 4] = ["statistic", "column", "value", "word"];

fn row(out: &mut String, statistic: &str, column: &str, value: &str, word: &str) {
    let fields = [text(statistic), text(column), value.to_string(), text(word)];
    out.push_str(&fields.join(","));
    out.push_str("\r\n");
}

fn number(out: &mut String, statistic: &str, column: &str, v: f64) {
    row(out, statistic, column, &format!("{v}"), "");
}

fn start() -> String {
    let mut out = HEADER.map(text).join(",");
    out.push_str("\r\n");
    out
}

/// The rows left out, then the limitations.
fn tail(out: &mut String, exclusions: &[Exclusion<OneSampleCode>], limitations: &[OneSampleCode]) {
    for e in exclusions {
        row(out, "excluded", "", &e.count.to_string(), e.code.wire());
    }
    for code in limitations {
        row(
            out,
            "limitation",
            "",
            "",
            <OneSampleCode as Closed>::wire(*code),
        );
    }
}

/// The one-sample result as CSV bytes (UTF-8, no byte-order mark).
pub fn one_sample_result_csv(result: &OneSampleResult) -> Vec<u8> {
    let mut out = start();
    row(&mut out, "n", "", &result.n.to_string(), "");
    for (name, v) in [
        ("mean", result.mean),
        ("sd", result.sd),
        ("test_value", result.test_value),
        ("t", result.t),
        ("df", result.df),
        ("p", result.p),
        ("mean_difference", result.mean_difference),
        ("standard_error", result.standard_error),
        ("confidence_level", result.confidence_level),
        ("ci_lower", result.ci_lower),
        ("ci_upper", result.ci_upper),
        ("cohens_d", result.cohens_d),
    ] {
        number(&mut out, name, "", v);
    }
    tail(&mut out, &result.exclusions, &result.limitations);
    out.into_bytes()
}

/// The paired result as CSV bytes (UTF-8, no byte-order mark).
pub fn paired_result_csv(result: &PairedResult) -> Vec<u8> {
    let mut out = start();
    row(&mut out, "n_pairs", "", &result.n_pairs.to_string(), "");
    for (column, c) in [("first", result.first), ("second", result.second)] {
        number(&mut out, "mean", column, c.mean);
        number(&mut out, "sd", column, c.sd);
    }
    for (name, v) in [
        ("t", result.t),
        ("df", result.df),
        ("p", result.p),
        ("mean_difference", result.mean_difference),
        ("sd_differences", result.sd_differences),
        ("standard_error", result.standard_error),
        ("confidence_level", result.confidence_level),
        ("ci_lower", result.ci_lower),
        ("ci_upper", result.ci_upper),
        ("cohens_dz", result.cohens_dz),
    ] {
        number(&mut out, name, "", v);
    }
    tail(&mut out, &result.exclusions, &result.limitations);
    out.into_bytes()
}

/// The manifest as JSON bytes, indented for a reader.
pub fn manifest_json(manifest: &RunManifest<OneSampleCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
