//! **The related rank tests' exports: the result as CSV bytes, the manifest as
//! JSON bytes (Research OS, Phase 8Z-1, decision 638).** Bytes only: the user's
//! save dialog and the frontend's existing save path write them, to the one file
//! the user names (decision 5, ADR-004a). Nothing here touches a file.
//!
//! The CSV is long, one row per number, in the eight fields every method's export
//! has: each chosen column's median over the complete rows; the rows used; then
//! the Wilcoxon signed-rank test, each line naming the first column and the
//! second, or the Friedman test, each column's rank sum and mean rank, then Q, its
//! df, its p and Kendall's W; then the rows left out, the notes, the limitations
//! and the manifest's hashes.
//!
//! ```text
//! "table","statistic","column_index","column","column_index_2","column_2","value","word"
//! "columns","median","0","C2","","",5.25,""
//! "rows","used","","","","",25,""
//! "wilcoxon","v","0","C2","1","C3",40,""
//! "wilcoxon","exact_p","0","C2","1","C3",,"stats.related_ranks.exact_not_given_ties"
//! "friedman","q","","","","",,"stats.related_ranks.all_values_tied"
//! "excluded","rows","","","","",3,"stats.related_ranks.missing_code"
//! "excluded","column","","","","",2,"stats.related_ranks.missing_code"
//! "manifest","output_sha256","","","","",,"9f…"
//! ```
//!
//! A column is its index in the list the user chose and its alias, never a
//! header. A value that is not defined is an empty field with its closed code in
//! `word`; an absent exact p carries the first note that says why, and a side with
//! no difference has an empty mean. Each row left out is two lines: its count, and
//! the column it was found in, by its alias's number (C2 is 2), as the rank tests'
//! export gives it.
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule.

use super::{RelatedRanksCode, RelatedRanksOutcome, RelatedTest, SignRanks, Stat};
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

/// A line's columns: each an index in the list and its alias, or empty.
struct At {
    first: Option<(usize, Alias)>,
    second: Option<(usize, Alias)>,
}

const NOWHERE: At = At {
    first: None,
    second: None,
};

fn line(out: &mut String, table: &str, statistic: &str, at: &At, value: &str, word: &str) {
    let column = |c: Option<(usize, Alias)>| match c {
        Some((i, alias)) => (text(&i.to_string()), text(&alias.0.name())),
        None => (text(""), text("")),
    };
    let (i1, a1) = column(at.first);
    let (i2, a2) = column(at.second);
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

fn number(out: &mut String, table: &str, statistic: &str, at: &At, v: f64) {
    line(out, table, statistic, at, &format!("{v}"), "");
}

fn whole(out: &mut String, table: &str, statistic: &str, at: &At, v: u64) {
    line(out, table, statistic, at, &v.to_string(), "");
}

fn code(out: &mut String, table: &str, statistic: &str, at: &At, c: RelatedRanksCode) {
    line(out, table, statistic, at, "", c.wire());
}

fn stat(out: &mut String, table: &str, statistic: &str, at: &At, s: Stat) {
    match s {
        Stat::Value(v) => number(out, table, statistic, at, v),
        Stat::NotComputable(c) => code(out, table, statistic, at, c),
    }
}

fn side(out: &mut String, at: &At, name: &str, s: &SignRanks) {
    whole(out, "wilcoxon", &format!("{name}_count"), at, s.count);
    number(out, "wilcoxon", &format!("{name}_rank_sum"), at, s.rank_sum);
    let statistic = format!("{name}_mean_rank");
    match s.mean_rank {
        Some(m) => number(out, "wilcoxon", &statistic, at, m),
        None => line(out, "wilcoxon", &statistic, at, "", ""),
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark), closed by `manifest`'s
/// hashes: the manifest of the run that produced `result`.
pub fn result_csv(
    result: &RelatedRanksOutcome,
    manifest: &RunManifest<RelatedRanksCode>,
) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    for (i, c) in result.columns.iter().enumerate() {
        let at = At {
            first: Some((i, c.column)),
            second: None,
        };
        number(&mut out, "columns", "median", &at, c.median);
    }
    whole(&mut out, "rows", "used", &NOWHERE, result.rows_used);
    match &result.test {
        RelatedTest::Wilcoxon(t) => {
            let table = "wilcoxon";
            let at = At {
                first: Some((0, t.first)),
                second: Some((1, t.second)),
            };
            whole(&mut out, table, "n", &at, t.n);
            whole(&mut out, table, "zero_differences", &at, t.zero_differences);
            whole(&mut out, table, "tie_runs", &at, t.tie_runs);
            side(&mut out, &at, "negative", &t.negative);
            side(&mut out, &at, "positive", &t.positive);
            number(&mut out, table, "v", &at, t.v);
            number(&mut out, table, "t_minus", &at, t.t_minus);
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
                            matches!(
                                c,
                                RelatedRanksCode::ExactNotGivenTies
                                    | RelatedRanksCode::ExactNotGivenZeros
                                    | RelatedRanksCode::ExactNotGivenSize
                            )
                        })
                        .unwrap_or(RelatedRanksCode::ExactWithoutTiesOrZerosOnly);
                    code(&mut out, table, "exact_p", &at, why);
                }
            }
            stat(&mut out, table, "rank_biserial", &at, t.rank_biserial);
            stat(&mut out, table, "median_difference", &at, t.median_difference);
        }
        RelatedTest::Friedman(t) => {
            let table = "friedman";
            for (i, c) in t.columns.iter().enumerate() {
                let at = At {
                    first: Some((i, c.column)),
                    second: None,
                };
                number(&mut out, table, "rank_sum", &at, c.rank_sum);
                number(&mut out, table, "mean_rank", &at, c.mean_rank);
            }
            stat(&mut out, table, "q", &NOWHERE, t.q);
            number(&mut out, table, "df", &NOWHERE, t.df);
            stat(&mut out, table, "p", &NOWHERE, t.p);
            stat(&mut out, table, "kendalls_w", &NOWHERE, t.kendalls_w);
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
        line(&mut out, "manifest", "input_sha256", &NOWHERE, "", h.as_hex());
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
pub fn manifest_json(manifest: &RunManifest<RelatedRanksCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
