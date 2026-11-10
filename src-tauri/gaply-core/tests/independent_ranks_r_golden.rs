//! **`gaply_core::stats_ranks` against R 4.6.1 (Research OS, Phase 8Y-1,
//! decisions 602 and 604), over `tests/fixtures/stats_ranks/` (written by
//! `/usr/local/bin/Rscript make_ranks_fixtures.R`).**
//!
//! Every number of every case is compared with R's, and each family has its
//! gate, set at ten times the largest error measured (decision 316), printed
//! with the measurement:
//!
//! ```text
//! cargo test -p gaply_core --test independent_ranks_r_golden -- --nocapture
//! ```
//!
//! R's oracles: `rank(ties.method = "average")` (each group's rank sum and
//! mean rank), `median`, `table` (the tie runs), `wilcox.test(exact = FALSE,
//! correct = FALSE)` (U1 as W, and the asymptotic p), `wilcox.test(exact =
//! TRUE)` (the exact p, asked for only without ties), `kruskal.test` (df and
//! p). z, the rank-biserial correlation, H in the form decision 618 fixes and
//! epsilon-squared have no base function: the same formulas are written
//! twice, in the generator and in `stats_ranks`. Gaply's H is compared with
//! that form, its p with `kruskal.test`'s, and with `kruskal.test`'s H at its
//! own gate in every case but `kw_large`, where `kruskal.test`'s form loses
//! digits and the difference is printed. Counts, df, levels, labels, the
//! first and second groups, exclusions and notes are compared exactly. Every
//! p value is compared by a pure relative error, as
//! `tests/stats_dist_reference.rs` does; every other number relative below a
//! floor of 1e-12, as `tests/crosstabs_r_golden.rs` does.
//!
//! A number R writes as `nc <code>` must be not computable with that code;
//! an exact p R writes as `absent` must be absent. The refusal cases name a
//! code and its parameters, and the first error of `compute` must be that
//! code with those parameters.

use std::collections::BTreeMap;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ParamKey, ParamValue};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_ranks::{
    compute, spec, validate, IndependentRanksOutcome, IndependentRanksParams, RankTest, RanksCode,
    Stat,
};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_ranks");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_ranks_fixtures.R";
const R_VERSION: &str = "R version 4.6.1";

/// The gates, each ten times the largest error measured over every case
/// (the measurement beside it, 10 Nov 2026, macOS aarch64). A family
/// measured at 0, bit for bit in every case, has a floor of 1e-15, as
/// crosstabs' Yates family has.
const GATES: &[(&str, f64)] = &[
    ("groups", 1e-15), // 0 measured: medians, rank sums and mean ranks bit for bit
    ("u", 1e-15),      // 0 measured: U1 and U2 bit for bit
    ("z", 1e-15),      // 0 measured: the formula written twice, bit for bit
    ("p", 9e-10),      // 8.833e-11, mw_large mann_whitney.p, pure relative (statrs's erfc against R's pnorm, as corr_r_golden.rs's 9.958e-11)
    ("h", 1e-15),      // 0 measured: H's form written twice, bit for bit
    ("h_kruskal", 1.2e-14), // 1.151e-15, kw_small: Gaply's H against kruskal.test's, every case but kw_large
    ("effect", 1e-15), // 0 measured: the rank-biserial correlation and epsilon-squared bit for bit
];

/// The keys compared exactly: whole numbers, labels and indexes.
fn exact(key: &str) -> bool {
    key.ends_with(".n")
        || key.ends_with(".df")
        || key.ends_with(".label")
        || key.starts_with("excluded.")
        || key.starts_with("note.")
        || matches!(
            key,
            "n" | "groups" | "tie_runs" | "mann_whitney.first" | "mann_whitney.second"
        )
}

fn family(key: &str) -> &'static str {
    let tail = key.rsplit('.').next().unwrap_or("");
    if tail == "p" || tail == "exact_p" {
        "p"
    } else if key.starts_with("group.") {
        "groups"
    } else if matches!(tail, "u1" | "u2") {
        "u"
    } else if tail == "z" {
        "z"
    } else if tail == "h" {
        "h"
    } else {
        "effect"
    }
}

/// A p value's error is purely relative; every other number's is relative
/// below a floor of 1e-12, where R's value is rounding noise about 0.
fn error(family: &str, got: f64, want: f64) -> f64 {
    if got == want {
        0.0
    } else if family == "p" {
        if want == 0.0 {
            got.abs()
        } else {
            (got - want).abs() / want.abs()
        }
    } else {
        (got - want).abs() / want.abs().max(1e-12)
    }
}

struct Case {
    name: String,
    file: String,
    outcome: String,
    group: String,
    levels: Option<Vec<u32>>,
    missing_codes: Vec<f64>,
}

fn table(name: &str) -> Vec<Vec<String>> {
    let text =
        std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let stamp: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(
        stamp.iter().any(|l| l.contains(PROVENANCE)),
        "{name}: not written by {PROVENANCE}"
    );
    assert!(
        stamp.iter().any(|l| l.contains(R_VERSION)),
        "{name}: not written with {R_VERSION}"
    );
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

fn cases() -> Vec<Case> {
    table("cases.tsv")
        .into_iter()
        .map(|f| Case {
            name: f[0].clone(),
            file: f[1].clone(),
            outcome: f[2].clone(),
            group: f[3].clone(),
            levels: if f[4] == "all" {
                None
            } else {
                Some(f[4].split(' ').map(|v| v.parse().unwrap()).collect())
            },
            missing_codes: if f[5] == "-" {
                Vec::new()
            } else {
                f[5].split(' ').map(|v| v.parse().unwrap()).collect()
            },
        })
        .collect()
}

fn expected() -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for f in table("expected.tsv") {
        out.entry(f[0].clone())
            .or_default()
            .insert(f[1].clone(), f[2].clone());
    }
    out
}

/// A case's dataset: its data file, or its counts expanded row by row, each
/// line in the file's order repeated n times, as the generator expands it.
fn data(case: &Case) -> Dataset {
    let bytes = std::fs::read(format!("{DIR}/{}", case.file)).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let header = lines.next().expect("header");
    let rows: Vec<Vec<String>> = if case.file.starts_with("counts_") {
        assert_eq!(header, "y,g,n", "{}", case.file);
        let mut rows = Vec::new();
        for l in lines {
            let f: Vec<&str> = l.split(',').collect();
            for _ in 0..f[2].parse::<u64>().expect("a count") {
                rows.push(vec![
                    (rows.len() + 1).to_string(),
                    f[0].to_string(),
                    f[1].to_string(),
                ]);
            }
        }
        rows
    } else {
        assert_eq!(header, "id,y,g", "{}", case.file);
        lines
            .map(|l| l.split(',').map(str::to_string).collect())
            .collect()
    };
    build_dataset(
        RawTable::from_rows(vec!["id".into(), "y".into(), "g".into()], rows),
        DataFingerprint::of_bytes(&bytes, "ranks golden", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn alias(d: &Dataset, header: &str) -> Alias {
    match d.schema.columns.iter().find(|c| c.header == header) {
        Some(c) => Alias(c.alias),
        // A column the dataset does not have: the refusal case's.
        None => Alias(ColumnAlias(99)),
    }
}

enum Got {
    Number(f64),
    Text(String),
    Nc(String),
    Absent,
}

fn stat(s: Stat) -> Got {
    match s {
        Stat::Value(v) => Got::Number(v),
        Stat::NotComputable(c) => Got::Nc(c.wire().to_string()),
    }
}

fn keys(r: &IndependentRanksOutcome) -> BTreeMap<String, Got> {
    let mut m = BTreeMap::new();
    m.insert("n".into(), Got::Number(r.n as f64));
    m.insert("groups".into(), Got::Number(r.groups.len() as f64));
    m.insert("tie_runs".into(), Got::Number(r.tie_runs as f64));
    for g in &r.groups {
        let key = |s: &str| format!("group.{}.{s}", g.level);
        m.insert(key("label"), Got::Text(g.label.clone()));
        m.insert(key("n"), Got::Number(g.n as f64));
        m.insert(key("median"), Got::Number(g.median));
        m.insert(key("rank_sum"), Got::Number(g.rank_sum));
        m.insert(key("mean_rank"), Got::Number(g.mean_rank));
    }
    match r.test {
        RankTest::MannWhitney(t) => {
            m.insert("mann_whitney.first".into(), Got::Number(f64::from(t.first)));
            m.insert("mann_whitney.second".into(), Got::Number(f64::from(t.second)));
            m.insert("mann_whitney.u1".into(), Got::Number(t.u1));
            m.insert("mann_whitney.u2".into(), Got::Number(t.u2));
            m.insert("mann_whitney.z".into(), stat(t.z));
            m.insert("mann_whitney.p".into(), stat(t.p));
            m.insert(
                "mann_whitney.exact_p".into(),
                t.exact_p.map_or(Got::Absent, Got::Number),
            );
            m.insert("mann_whitney.rank_biserial".into(), Got::Number(t.rank_biserial));
        }
        RankTest::KruskalWallis(t) => {
            m.insert("kruskal_wallis.h".into(), stat(t.h));
            m.insert("kruskal_wallis.df".into(), Got::Number(t.df));
            m.insert("kruskal_wallis.p".into(), stat(t.p));
            m.insert("kruskal_wallis.epsilon_squared".into(), stat(t.epsilon_squared));
        }
    }
    for (code, key) in [
        (RanksCode::ExactNotGivenTies, "note.exact_not_given_ties"),
        (RanksCode::ExactNotGivenSize, "note.exact_not_given_size"),
        (RanksCode::GroupBelowFive, "note.group_below_five"),
    ] {
        m.insert(
            key.into(),
            Got::Number(if r.notes.contains(&code) { 1.0 } else { 0.0 }),
        );
    }
    let mut excluded: BTreeMap<String, f64> = [
        "excluded.missing_group",
        "excluded.other_level",
        "excluded.missing_outcome",
        "excluded.missing_code",
    ]
    .into_iter()
    .map(|k| (k.to_string(), 0.0))
    .collect();
    for e in &r.exclusions {
        let key = format!("excluded.{}", e.code.wire().trim_start_matches("stats.ranks."));
        assert!(excluded.contains_key(&key), "unknown exclusion {key}");
        excluded.insert(key, e.count as f64);
    }
    for (k, v) in excluded {
        m.insert(k, Got::Number(v));
    }
    m
}

#[derive(Default)]
struct Worst(f64, String);

fn param(e: &gaply_core::contracts::Diagnostic<RanksCode>, key: ParamKey) -> Option<i64> {
    match e.params.get(&key) {
        Some(ParamValue::Int(v)) => Some(*v),
        _ => None,
    }
}

#[test]
fn every_number_of_every_case_is_within_its_gate_of_r() {
    let want_all = expected();
    let cases = cases();
    assert_eq!(cases.len(), 30, "the generator's cases");
    let mut worst: BTreeMap<&str, Worst> = BTreeMap::new();
    let mut over = Vec::new();
    let mut compared = 0usize;
    let (mut refusals, mut results, mut not_computable, mut absent) = (0, 0, 0, 0);
    let mut h_forms = Worst::default();
    for case in &cases {
        let want = want_all
            .get(&case.name)
            .unwrap_or_else(|| panic!("{}: no expected values", case.name));
        let d = data(case);
        let (outcome, group) = (alias(&d, &case.outcome), alias(&d, &case.group));
        let params = IndependentRanksParams {
            outcome,
            group,
            levels: case.levels.clone(),
            missing_codes: case.missing_codes.clone(),
        };
        if let Some(code) = want.get("refused") {
            refusals += 1;
            let e = compute(&d, &params)
                .err()
                .unwrap_or_else(|| panic!("{}: R refuses with {code}, gaply computed", case.name));
            assert_eq!(&e.code.wire().to_string(), code, "{}: the first refusal", case.name);
            let report = validate(&d, &spec(OperationId(1), &d, params.clone()));
            assert_eq!(
                report.errors.first().map(|x| x.code.wire().to_string()),
                Some(code.clone()),
                "{}: validate's first error is run's",
                case.name
            );
            for (key, pk) in [
                ("refused.index", ParamKey::Index),
                ("refused.count", ParamKey::Count),
                ("refused.limit", ParamKey::Limit),
            ] {
                if let Some(v) = want.get(key) {
                    assert_eq!(param(&e, pk), Some(v.parse::<i64>().unwrap()), "{} {key}", case.name);
                }
            }
            if let Some(which) = want.get("refused.column") {
                let a = if which == "outcome" { outcome } else { group };
                assert_eq!(param(&e, ParamKey::Column), Some(i64::from(a.0 .0)), "{} column", case.name);
            }
            continue;
        }
        results += 1;
        let r = compute(&d, &params).unwrap_or_else(|e| panic!("{}: refused {e:?}", case.name));
        let got = keys(&r);
        for (key, w) in want {
            if key == "kruskal_wallis.h_kruskal_test" {
                // R's own form: gated, but in kw_large, where its form cancels
                // and the difference is printed (decision 618).
                let Some(Got::Number(gaply)) = got.get("kruskal_wallis.h") else {
                    panic!("{}: no H beside kruskal.test's", case.name);
                };
                let other: f64 = w.parse().unwrap();
                let diff = (gaply - other).abs() / other.abs();
                println!("{}: H, gaply {gaply:e}, kruskal.test {other:e}, relative difference {diff:.3e}", case.name);
                if case.name == "kw_large" {
                    h_forms = Worst(diff, case.name.clone());
                    continue;
                }
                let gate = GATES.iter().find(|(f, _)| *f == "h_kruskal").unwrap().1;
                if diff > gate {
                    over.push(format!("{} {key}: gaply {gaply:e} R {other:e} error {diff:e} gate {gate:e}", case.name));
                }
                let w = worst.entry("h_kruskal").or_default();
                if diff >= w.0 {
                    *w = Worst(diff, format!("{} {key}", case.name));
                }
                continue;
            }
            let g = got
                .get(key)
                .unwrap_or_else(|| panic!("{}: the result has no {key}", case.name));
            match g {
                Got::Text(t) => assert_eq!(t, w, "{} {key}", case.name),
                Got::Nc(c) => {
                    not_computable += 1;
                    assert_eq!(&format!("nc {c}"), w, "{} {key}: not computable", case.name);
                }
                Got::Absent => {
                    absent += 1;
                    assert_eq!(w, "absent", "{} {key}: absent where R gives {w}", case.name);
                }
                Got::Number(v) => {
                    let wv: f64 = w
                        .parse()
                        .unwrap_or_else(|_| panic!("{} {key}: gaply {v} where R gives {w}", case.name));
                    compared += 1;
                    if exact(key) {
                        assert_eq!(*v, wv, "{} {key}", case.name);
                        continue;
                    }
                    let fam = family(key);
                    let err = error(fam, *v, wv);
                    let gate = GATES.iter().find(|(f, _)| *f == fam).unwrap().1;
                    if err > gate {
                        over.push(format!("{} {key}: gaply {v:e} R {wv:e} error {err:e} gate {gate:e}", case.name));
                    }
                    let w = worst.entry(fam).or_default();
                    if err >= w.0 {
                        *w = Worst(err, format!("{} {key}", case.name));
                    }
                }
            }
        }
        // And the result carries no number R does not have.
        for key in got.keys() {
            assert!(want.contains_key(key), "{}: {key} has no R value", case.name);
        }
    }
    for (fam, gate) in GATES {
        let w = worst.get(fam).map(|w| (w.0, w.1.as_str())).unwrap_or((0.0, "none"));
        println!("{fam:<7} largest error {:.3e} ({})  gate {gate:e}", w.0, w.1);
    }
    println!("kw_large: Gaply's H against kruskal.test's, relative difference {:.3e} (printed, not gated)", h_forms.0);
    assert_eq!(h_forms.1, "kw_large", "kw_large's difference was read");
    println!("{results} results, {refusals} refusals, {compared} numbers compared, {not_computable} not computable, {absent} exact p absent");
    assert_eq!((results, refusals), (19, 11));
    assert_eq!(not_computable, 5, "all_equal_two's z and p, all_equal_three's H, p and epsilon-squared");
    assert!(absent >= 4, "{absent} exact p absent");
    assert!(over.is_empty(), "over the gate:\n{}", over.join("\n"));
}
