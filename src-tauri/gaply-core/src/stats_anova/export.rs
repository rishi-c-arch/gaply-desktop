//! **The one-way ANOVA's exports: the result as CSV bytes, the manifest as
//! JSON bytes (Research OS, Phase 8G-2, decision 327).** Bytes only: the
//! user's save dialog and the frontend's existing save path write them, to
//! the one file the user names (decision 5, ADR-004a). Nothing here touches
//! a file.
//!
//! The CSV is long, one row per number, every table in the order the screen
//! shows it: the group table, the ANOVA table, Welch's ANOVA, Levene's two
//! lines, the effect sizes, then each chosen post hoc test's pairs; then the
//! rows left out, the notes, the limitations and the manifest's hashes.
//!
//! ```text
//! "table","statistic","level_index","level","level_index_2","level_2","value","word"
//! "result","confidence_level","","","","",0.95,""
//! "groups","mean","0","ctrl","","",2.5,""
//! "anova","f","","","","",7.25,""
//! "welch","not_computable","","","","",,"stats.anova.welch_variance_zero"
//! "tukey","statistic","0","ctrl","1","dose",4.1,"q"
//! "tukey","p","0","ctrl","2","high",0.0000000001,"below"
//! "excluded","rows","","","","",3,"stats.anova.missing_outcome"
//! "manifest","output_sha256","","","","",,"9f…"
//! ```
//!
//! A pair is its first level minus its second, each by its index in the
//! group's code-point order and its label. A value that is not defined is an
//! empty field with its closed code in `word`; a studentized-range p below
//! the floor it is computed to is written as the floor with the word
//! `below`, never as a number of its own (decision 314).
//!
//! The rules are the descriptive summary's, through its own field writer
//! (`stats_desc::export::text`), so there is one formula-injection rule: RFC
//! 4180 with CRLF line ends; every text field quoted, an inner `"` doubled;
//! numbers unquoted in Rust's shortest form that reads back to the same
//! double; a text field that starts with `=`, `+`, `-`, `@`, a tab, a carriage
//! return or an apostrophe gets one leading apostrophe.

use super::{
    AnovaCode, AnovaOutcome, FOutcome, PairOutcome, PostHoc, PostHocP, PostHocPairs, Stat, Table,
    CONFIDENCE_LEVEL,
};
use crate::contracts::codes::Closed;
use crate::contracts::RunManifest;
use crate::stats_desc::export::text;

const HEADER: [&str; 8] = [
    "table",
    "statistic",
    "level_index",
    "level",
    "level_index_2",
    "level_2",
    "value",
    "word",
];

/// A row's two levels: each an index and its label, or empty.
struct Levels<'a> {
    first: Option<(u32, &'a str)>,
    second: Option<(u32, &'a str)>,
}

const NO_LEVELS: Levels<'static> = Levels {
    first: None,
    second: None,
};

fn row(out: &mut String, table: &str, statistic: &str, levels: &Levels, value: &str, word: &str) {
    let level = |l: Option<(u32, &str)>| match l {
        Some((i, label)) => (text(&i.to_string()), text(label)),
        None => (text(""), text("")),
    };
    let (i1, l1) = level(levels.first);
    let (i2, l2) = level(levels.second);
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

fn number(out: &mut String, table: &str, statistic: &str, levels: &Levels, v: f64) {
    row(out, table, statistic, levels, &format!("{v}"), "");
}

fn code(out: &mut String, table: &str, statistic: &str, levels: &Levels, c: AnovaCode) {
    row(out, table, statistic, levels, "", c.wire());
}

fn stat(out: &mut String, table: &str, statistic: &str, s: Stat) {
    match s {
        Stat::Value(v) => number(out, table, statistic, &NO_LEVELS, v),
        Stat::NotComputable(c) => code(out, table, statistic, &NO_LEVELS, c),
    }
}

fn test_word(t: PostHoc) -> (&'static str, &'static str) {
    match t {
        PostHoc::Tukey => ("tukey", "q"),
        PostHoc::GamesHowell => ("games_howell", "q"),
        PostHoc::Bonferroni => ("bonferroni", "t"),
    }
}

/// The result as CSV bytes (UTF-8, no byte-order mark), closed by
/// `manifest`'s hashes: the manifest of the run that produced `result`.
pub fn result_csv(result: &AnovaOutcome, manifest: &RunManifest<AnovaCode>) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(&HEADER.map(text).join(","));
    out.push_str("\r\n");
    number(
        &mut out,
        "result",
        "confidence_level",
        &NO_LEVELS,
        CONFIDENCE_LEVEL,
    );
    for g in &result.groups {
        let at = Levels {
            first: Some((g.level, g.label.as_str())),
            second: None,
        };
        row(&mut out, "groups", "n", &at, &g.n.to_string(), "");
        for (name, v) in [
            ("mean", g.mean),
            ("sd", g.sd),
            ("se", g.se),
            ("ci_lower", g.ci_lower),
            ("ci_upper", g.ci_upper),
            ("min", g.min),
            ("max", g.max),
        ] {
            number(&mut out, "groups", name, &at, v);
        }
    }
    match result.table {
        Table::Computed(t) => {
            for (name, v) in [
                ("ss_between", t.ss_between),
                ("ss_within", t.ss_within),
                ("ss_total", t.ss_total),
                ("df_between", t.df_between),
                ("df_within", t.df_within),
                ("ms_between", t.ms_between),
                ("ms_within", t.ms_within),
            ] {
                number(&mut out, "anova", name, &NO_LEVELS, v);
            }
            stat(&mut out, "anova", "f", t.f);
            stat(&mut out, "anova", "p", t.p);
        }
        Table::NotComputable(c) => code(&mut out, "anova", "not_computable", &NO_LEVELS, c),
    }
    for (name, test) in [
        ("welch", result.welch),
        ("levene_mean", result.levene_mean),
        ("levene_median", result.levene_median),
    ] {
        match test {
            FOutcome::Computed(t) => {
                for (s, v) in [("f", t.f), ("df1", t.df1), ("df2", t.df2), ("p", t.p)] {
                    number(&mut out, name, s, &NO_LEVELS, v);
                }
            }
            FOutcome::NotComputable(c) => code(&mut out, name, "not_computable", &NO_LEVELS, c),
        }
    }
    stat(&mut out, "effect_sizes", "eta_squared", result.eta_squared);
    stat(
        &mut out,
        "effect_sizes",
        "omega_squared",
        result.omega_squared,
    );
    let label = |level: u32| -> &str {
        result
            .groups
            .iter()
            .find(|g| g.level == level)
            .map(|g| g.label.as_str())
            .unwrap_or("")
    };
    for t in &result.post_hoc {
        let (name, statistic) = test_word(t.test);
        let pairs = match &t.pairs {
            PostHocPairs::Computed(pairs) => pairs,
            PostHocPairs::NotComputable(c) => {
                code(&mut out, name, "not_computable", &NO_LEVELS, *c);
                continue;
            }
        };
        for p in pairs {
            let at = Levels {
                first: Some((p.first, label(p.first))),
                second: Some((p.second, label(p.second))),
            };
            match p.outcome {
                PairOutcome::Computed(s) => {
                    number(&mut out, name, "difference", &at, s.difference);
                    number(&mut out, name, "se", &at, s.se);
                    row(
                        &mut out,
                        name,
                        "statistic",
                        &at,
                        &format!("{}", s.statistic),
                        statistic,
                    );
                    number(&mut out, name, "df", &at, s.df);
                    match s.p {
                        PostHocP::Value(v) => number(&mut out, name, "p", &at, v),
                        PostHocP::Below(floor) => {
                            row(&mut out, name, "p", &at, &format!("{floor}"), "below")
                        }
                    }
                    number(&mut out, name, "ci_lower", &at, s.ci_lower);
                    number(&mut out, name, "ci_upper", &at, s.ci_upper);
                }
                PairOutcome::NotComputable(c) => code(&mut out, name, "not_computable", &at, c),
            }
        }
    }
    for e in &result.exclusions {
        row(
            &mut out,
            "excluded",
            "rows",
            &NO_LEVELS,
            &e.count.to_string(),
            e.code.wire(),
        );
    }
    for c in &result.notes {
        code(&mut out, "note", "", &NO_LEVELS, *c);
    }
    for c in &result.limitations {
        code(&mut out, "limitation", "", &NO_LEVELS, *c);
    }
    for h in &manifest.inputs {
        row(
            &mut out,
            "manifest",
            "input_sha256",
            &NO_LEVELS,
            "",
            h.as_hex(),
        );
    }
    row(
        &mut out,
        "manifest",
        "parameters_sha256",
        &NO_LEVELS,
        "",
        manifest.parameters_hash.as_hex(),
    );
    row(
        &mut out,
        "manifest",
        "output_sha256",
        &NO_LEVELS,
        "",
        manifest.output_hash.as_hex(),
    );
    out.into_bytes()
}

/// The manifest as JSON bytes, indented for a reader.
pub fn manifest_json(manifest: &RunManifest<AnovaCode>) -> Vec<u8> {
    serde_json::to_vec_pretty(manifest).expect("a manifest of hashes, words and numbers serialises")
}
