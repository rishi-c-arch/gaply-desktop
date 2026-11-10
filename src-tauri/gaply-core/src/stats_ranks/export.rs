//! **The rank tests' exports: the result as CSV bytes, the manifest as JSON
//! bytes (Research OS, Phase 8Y-1, decision 604).** Bytes only: the user's
//! save dialog and the frontend's existing save path write them, to the one
//! file the user names (decision 5, ADR-004a). Nothing here touches a file.
//!
//! The CSV is long, one row per number, in the eight fields every method's
//! export has: each included group's count, median, rank sum and mean rank;
//! the included values and their tie runs; then the Mann-Whitney test (U1,
//! U2, z, its p, the exact p and the rank-biserial correlation, each line
//! naming the first group and the second) or the Kruskal-Wallis test (H, its
//! df, its p and epsilon-squared); then the rows left out, the notes, the
//! limitations and the manifest's hashes.
//!
//! ```text
//! "table","statistic","group_index","group","group_index_2","group_2","value","word"
//! "groups","n","0","control","","",12,""
//! "groups","mean_rank","0","control","","",9.5,""
//! "ranks","n","","","","",25,""
//! "mann_whitney","u1","0","control","1","treated",40,""
//! "mann_whitney","exact_p","0","control","1","treated",,"stats.ranks.exact_not_given_ties"
//! "kruskal_wallis","h","","","","",,"stats.ranks.all_values_tied"
//! "excluded","rows","","","","",3,"stats.ranks.missing_code"
//! "excluded","column","","","","",0,"stats.ranks.missing_code"
//! "manifest","output_sha256","","","","",,"9f…"
//! ```
//!
//! A group is its index in the group column's code-point order and its
//! label. A value that is not defined is an empty field with its closed
//! code in `word`; an absent exact p carries the note that says why, the
//! ties' before the size's. Each row left out is two lines: its count, and
//! the column it was found in, by the position the spec names it by.
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule: RFC
//! 4180 with CRLF line ends; every text field quoted, an inner `"` doubled;
//! numbers unquoted in Rust's shortest form that reads back to the same
//! double; a text field that starts with `=`, `+`, `-`, `@`, a tab, a carriage
//! return or an apostrophe gets one leading apostrophe.

use super::{IndependentRanksOutcome, RankTest, RanksCode, Stat};
use crate::contracts::codes::Closed;
use crate::contracts::{ParamKey, ParamValue, RunManifest};
use crate::stats_desc::export::text;

const HEADER: [&str; 8] = [
    "table",
    "statistic",
    "group_index",
    "group",
    "group_index_2",
    "group_2",
    "value",
    "word",
];

/// A line's groups: each an index and its label, or empty.
struct At<'a> {
    first: Option<(u32, &'a str)>,
    second: Option<(u32, &'a str)>,
}

const NOWHERE: At<'static> = At {
    first: None,
    second: None,
};

fn line(out: &mut String, table: &str, statistic: &str, at: &At, value: &str, word: &str) {
    let group = |g: Option<(u32, &str)>| match g {
        Some((i, label)) => (text(&i.to_string()), text(label)),
        None => (text(""), text("")),
    };
    let (i1, l1) = group(at.first);
    let (i2, l2) = group(at.second);
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

fn code(out: &mut String, table: &str, statistic: &str, at: &At, c: RanksCode) {
    line(out, table, statistic, at, "", c.wire());
}

fn stat(out: &mut String, table: &str, statistic: &str, at: &At, s: Stat) {
    match s {
        Stat::Value(v) => number(out, table, statistic, at, v),
        Stat::NotComputable(c) => code(out, table, statistic, at, c),
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark), closed by
/// `manifest`'s hashes: the manifest of the run that produced `result`.
pub fn result_csv(result: &IndependentRanksOutcome, manifest: &RunManifest<RanksCode>) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    let label = |level: u32| {
        result
            .groups
            .iter()
            .find(|g| g.level == level)
            .map(|g| g.label.as_str())
            .unwrap_or("")
    };
    for g in &result.groups {
        let at = At {
            first: Some((g.level, g.label.as_str())),
            second: None,
        };
        whole(&mut out, "groups", "n", &at, g.n);
        number(&mut out, "groups", "median", &at, g.median);
        number(&mut out, "groups", "rank_sum", &at, g.rank_sum);
        number(&mut out, "groups", "mean_rank", &at, g.mean_rank);
    }
    whole(&mut out, "ranks", "n", &NOWHERE, result.n);
    whole(&mut out, "ranks", "tie_runs", &NOWHERE, result.tie_runs);
    match result.test {
        RankTest::MannWhitney(t) => {
            let table = "mann_whitney";
            let at = At {
                first: Some((t.first, label(t.first))),
                second: Some((t.second, label(t.second))),
            };
            number(&mut out, table, "u1", &at, t.u1);
            number(&mut out, table, "u2", &at, t.u2);
            stat(&mut out, table, "z", &at, t.z);
            stat(&mut out, table, "p", &at, t.p);
            match t.exact_p {
                Some(p) => number(&mut out, table, "exact_p", &at, p),
                None => {
                    let why = result
                        .notes
                        .iter()
                        .copied()
                        .find(|c| {
                            matches!(c, RanksCode::ExactNotGivenTies | RanksCode::ExactNotGivenSize)
                        })
                        .unwrap_or(RanksCode::ExactWithoutTiesOnly);
                    code(&mut out, table, "exact_p", &at, why);
                }
            }
            number(&mut out, table, "rank_biserial", &at, t.rank_biserial);
        }
        RankTest::KruskalWallis(t) => {
            let table = "kruskal_wallis";
            stat(&mut out, table, "h", &NOWHERE, t.h);
            number(&mut out, table, "df", &NOWHERE, t.df);
            stat(&mut out, table, "p", &NOWHERE, t.p);
            stat(&mut out, table, "epsilon_squared", &NOWHERE, t.epsilon_squared);
        }
    }
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
        code(&mut out, "note", "", &NOWHERE, *c);
    }
    for c in &result.limitations {
        code(&mut out, "limitation", "", &NOWHERE, *c);
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
pub fn manifest_json(manifest: &RunManifest<RanksCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
