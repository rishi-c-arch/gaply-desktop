//! **`gaply_core::stats_anova` against R 4.6.1 (Research OS, Phase 8G,
//! decisions 312 to 316), over `tests/fixtures/stats_anova/` (written by
//! `/usr/local/bin/Rscript make_anova_fixtures.R`).**
//!
//! Every statistic of every case is compared with R's, and each family has
//! its gate, set at ten times the largest error measured (decision 316),
//! printed with the measurement:
//!
//! ```text
//! cargo test -p gaply_core --test anova_r_golden -- --nocapture
//! ```
//!
//! R's oracles: `aov`, `oneway.test`, `car::leveneTest`, effectsize's
//! `eta_squared` and `omega_squared`, `TukeyHSD`, `pairwise.t.test`, and
//! Games–Howell from its formula with `ptukey` and `qtukey` (the same formula
//! written twice, the generator's header says so). A Games–Howell pair whose
//! df is below 2, where R's `ptukey` and `qtukey` return NaN, is checked only
//! for being computed: its p and interval rest on `stats_dist`'s gate against
//! mpmath (`tests/studentized_range_reference.rs`).
//!
//! The not-computable cases are decision 312's: each "NC" in the fixture
//! must be not computable here with the code the case names, and each
//! number must be a number.
//!
//! The 100,000-row case `big5` is built here from its formula, as the
//! generator builds it in R: row i in group i mod 5, y = ((i · 7919) mod
//! 10007) / 100 + (i mod 5) · 0.5, exact in both. Its post hoc tests are
//! not run here (the release speed measurement runs them); k50's Games–Howell
//! is not run here either, for the same reason: an unoptimised build takes
//! minutes over its 1,225 quantiles.

use std::collections::BTreeMap;

use gaply_core::contracts::OperationId;
use gaply_core::stats_anova::{
    compute, spec, validate, AnovaCode, AnovaOutcome, AnovaParams, FOutcome, PairOutcome, PostHoc,
    PostHocP, PostHocPairs, Stat, Table,
};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_anova");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_anova_fixtures.R";

/// The gates, each ten times the largest relative error measured over every
/// case (the measurement beside it, 31 Oct 2026, macOS aarch64).
const GATES: &[(&str, f64)] = &[
    ("group", 7e-14),   // 6.86e-15, k10 group.5.ci_lower
    ("anova", 1e-11),   // 9.33e-13, big5 anova.f
    ("anova.p", 6e-10), // 5.21e-11, big5 anova.p (p 5.1e-12)
    ("welch", 6e-12),   // 5.75e-13, big5 welch.p
    ("levene", 2e-10),  // 1.46e-11, big5 levene_mean.f
    ("effect", 2e-11),  // 1.00e-12, big5 omega_sq
    ("tukey", 7e-14),   // 6.22e-15, k10 tukey.3-6.q
    ("tukey.p", 7e-6),  // ABSOLUTE 6.39e-7, k50 tukey.1-20.p: R's ptukey
    ("tukey.ci", 2e-5), // 1.99e-6, n2 tukey.0-3.ci_upper: R's qtukey
    ("gh", 7e-14),      // 6.16e-15, k10 gh.3-6.q
    ("gh.p", 3e-5),     // ABSOLUTE 2.18e-6, some_constant gh.0-3.p: R's ptukey at df 4
    ("gh.ci", 2e-4),    // 1.70e-5, missing gh.0-2.ci_lower: R's qtukey
    ("bonf", 8e-12),    // 7.90e-13, codepoint bonf.0-4.ci_lower
    ("bonf.p", 1e-11),  // 9.89e-13, k3_unequal bonf.0-2.p
    ("offset", 3e-6),   // 2.62e-7, ties_offset levene_mean.p: means near 1e9 hold an ulp of 1.2e-7
];

fn family(key: &str) -> &'static str {
    let fam = |prefix: &str| key.starts_with(prefix);
    let tail = key.rsplit('.').next().unwrap_or("");
    let pick = |base: &'static str, p: &'static str, ci: &'static str| match tail {
        "p" => p,
        "ci_lower" | "ci_upper" | "crit" => ci,
        _ => base,
    };
    if fam("group.") {
        "group"
    } else if key == "anova.p" {
        "anova.p"
    } else if fam("anova.") {
        "anova"
    } else if fam("welch.") {
        "welch"
    } else if fam("levene") {
        "levene"
    } else if fam("eta") || fam("omega") {
        "effect"
    } else if fam("tukey.") {
        pick("tukey", "tukey.p", "tukey.ci")
    } else if fam("gh.") {
        pick("gh", "gh.p", "gh.ci")
    } else if fam("bonf.") {
        match tail {
            "p" => "bonf.p",
            _ => "bonf",
        }
    } else {
        "count"
    }
}

/// The error a family is gated on. The studentized-range p's against R are
/// ABSOLUTE: R's `ptukey` is 1 − cdf, so its own error is absolute (measured:
/// 1.5e-5 relative at a p of 2.7e-8, and 1.4e-3 relative at df 4, q 15.1,
/// where mpmath gives 0.00152328425027 and Gaply 0.00152328425028 against R's
/// 0.00152111); `stats_dist`'s own relative gate is against mpmath. Every
/// other family is relative, below a floor of 1e-12 where R's value is
/// rounding noise about 0 (aov's MS within of 3.0e-32 where every group is
/// constant; a Levene F of 1.7e-27 on deviations whose between-group sum is
/// 0 in exact arithmetic).
fn error(family: &str, got: f64, want: f64) -> f64 {
    if got == want {
        0.0
    } else if family == "tukey.p" || family == "gh.p" {
        (got - want).abs()
    } else {
        (got - want).abs() / want.abs().max(1e-12)
    }
}

struct Case {
    name: String,
    file: String,
    include: Option<Vec<u32>>,
    missing_codes: Vec<f64>,
}

fn table(name: &str) -> Vec<Vec<String>> {
    let text =
        std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(
        text.lines()
            .any(|l| l.starts_with('#') && l.contains(PROVENANCE)),
        "{name}: not written by {PROVENANCE}"
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
            include: (f[2] != "all").then(|| f[2].split(' ').map(|i| i.parse().unwrap()).collect()),
            missing_codes: if f[3] == "-" {
                Vec::new()
            } else {
                f[3].split(' ').map(|v| v.parse().unwrap()).collect()
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

fn dataset_from(headers: Vec<String>, rows: Vec<Vec<String>>, bytes: &[u8]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(bytes, "anova golden", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// A case's dataset: its CSV, or big5's formula.
fn data(case: &Case) -> Dataset {
    if case.file == "-" {
        assert_eq!(case.name, "big5");
        let rows: Vec<Vec<String>> = (0u64..100_000)
            .map(|i| {
                let y = ((i * 7919) % 10007) as f64 / 100.0 + (i % 5) as f64 * 0.5;
                vec![
                    (i + 1).to_string(),
                    format!("{y:?}"),
                    ["A", "B", "C", "D", "E"][(i % 5) as usize].to_string(),
                ]
            })
            .collect();
        return dataset_from(
            vec!["id".into(), "y".into(), "g".into()],
            rows,
            b"big5 by formula",
        );
    }
    let bytes = std::fs::read(format!("{DIR}/{}", case.file)).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines
        .next()
        .expect("header")
        .split(',')
        .map(str::to_string)
        .collect();
    assert_eq!(headers, ["id", "y", "g"], "{}", case.file);
    let rows = lines
        .map(|l| l.split(',').map(str::to_string).collect())
        .collect();
    dataset_from(headers, rows, &bytes)
}

fn alias(d: &Dataset, header: &str) -> Alias {
    Alias(
        d.schema
            .columns
            .iter()
            .find(|c| c.header == header)
            .unwrap_or_else(|| panic!("no column {header}"))
            .alias,
    )
}

/// The post hoc tests each case runs (the module doc says why big5 and
/// k50's Games–Howell are left to the release measurement).
fn post_hoc_for(case: &str) -> Vec<PostHoc> {
    match case {
        "big5" => Vec::new(),
        "k50" => vec![PostHoc::Tukey, PostHoc::Bonferroni],
        _ => vec![PostHoc::Tukey, PostHoc::GamesHowell, PostHoc::Bonferroni],
    }
}

/// What a result says for one fixture key: a number, a not-computable
/// code, or nothing (a key this result does not carry).
enum Got {
    Number(f64),
    Nc(AnovaCode),
    Below,
}

fn stat(s: Stat) -> Got {
    match s {
        Stat::Value(v) => Got::Number(v),
        Stat::NotComputable(c) => Got::Nc(c),
    }
}

fn keys(r: &AnovaOutcome) -> BTreeMap<String, Got> {
    let mut m = BTreeMap::new();
    for (i, g) in r.groups.iter().enumerate() {
        for (k, v) in [
            ("n", g.n as f64),
            ("mean", g.mean),
            ("sd", g.sd),
            ("se", g.se),
            ("ci_lower", g.ci_lower),
            ("ci_upper", g.ci_upper),
            ("min", g.min),
            ("max", g.max),
        ] {
            m.insert(format!("group.{i}.{k}"), Got::Number(v));
        }
    }
    match r.table {
        Table::Computed(t) => {
            for (k, v) in [
                ("ss_between", t.ss_between),
                ("ss_within", t.ss_within),
                ("ss_total", t.ss_total),
                ("df_between", t.df_between),
                ("df_within", t.df_within),
                ("ms_between", t.ms_between),
                ("ms_within", t.ms_within),
            ] {
                m.insert(format!("anova.{k}"), Got::Number(v));
            }
            m.insert("anova.f".into(), stat(t.f));
            m.insert("anova.p".into(), stat(t.p));
        }
        Table::NotComputable(c) => {
            for k in [
                "ss_between",
                "ss_within",
                "ss_total",
                "ms_between",
                "ms_within",
                "f",
                "p",
            ] {
                m.insert(format!("anova.{k}"), Got::Nc(c));
            }
        }
    }
    for (name, f) in [
        ("welch", r.welch),
        ("levene_mean", r.levene_mean),
        ("levene_median", r.levene_median),
    ] {
        for (k, v) in ["f", "df1", "df2", "p"].iter().zip(match f {
            FOutcome::Computed(t) => [Some(t.f), Some(t.df1), Some(t.df2), Some(t.p)],
            FOutcome::NotComputable(_) => [None; 4],
        }) {
            m.insert(
                format!("{name}.{k}"),
                match (v, f) {
                    (Some(v), _) => Got::Number(v),
                    (None, FOutcome::NotComputable(c)) => Got::Nc(c),
                    _ => unreachable!(),
                },
            );
        }
    }
    m.insert("eta_sq".into(), stat(r.eta_squared));
    m.insert("eta_sq_effectsize".into(), stat(r.eta_squared));
    m.insert("omega_sq".into(), stat(r.omega_squared));
    m.insert("omega_sq_effectsize".into(), stat(r.omega_squared));
    let position: BTreeMap<u32, usize> = r
        .groups
        .iter()
        .enumerate()
        .map(|(i, g)| (g.level, i))
        .collect();
    for table in &r.post_hoc {
        let prefix = match table.test {
            PostHoc::Tukey => "tukey",
            PostHoc::GamesHowell => "gh",
            PostHoc::Bonferroni => "bonf",
        };
        let PostHocPairs::Computed(pairs) = &table.pairs else {
            m.insert("posthoc".into(), Got::Nc(AnovaCode::ArithmeticRefused));
            continue;
        };
        for pair in pairs {
            let key = format!(
                "{prefix}.{}-{}",
                position[&pair.first], position[&pair.second]
            );
            match pair.outcome {
                PairOutcome::Computed(s) => {
                    let stat_key = if prefix == "bonf" { "t" } else { "q" };
                    for (k, v) in [
                        ("diff", s.difference),
                        ("se", s.se),
                        (stat_key, s.statistic),
                        ("df", s.df),
                        ("ci_lower", s.ci_lower),
                        ("ci_upper", s.ci_upper),
                    ] {
                        m.insert(format!("{key}.{k}"), Got::Number(v));
                    }
                    if prefix != "bonf" {
                        let crit = (s.ci_upper - s.difference) * std::f64::consts::SQRT_2 / s.se;
                        m.insert(format!("{key}.crit"), Got::Number(crit));
                    }
                    m.insert(
                        format!("{key}.p"),
                        match s.p {
                            PostHocP::Value(v) => Got::Number(v),
                            PostHocP::Below(_) => Got::Below,
                        },
                    );
                }
                PairOutcome::NotComputable(c) => {
                    for k in ["diff", "se", "q", "df", "p", "ci_lower", "ci_upper"] {
                        m.insert(format!("{key}.{k}"), Got::Nc(c));
                    }
                }
            }
        }
    }
    for e in &r.exclusions {
        let name = gaply_core::contracts::codes::Closed::wire(e.code);
        m.insert(
            format!("excluded.{}", name.trim_start_matches("stats.anova.")),
            Got::Number(e.count as f64),
        );
    }
    m.insert("k".into(), Got::Number(r.groups.len() as f64));
    m.insert(
        "rows_used".into(),
        Got::Number(r.groups.iter().map(|g| g.n as f64).sum()),
    );
    m
}

/// The not-computable code each case's "NC" keys must carry.
fn nc_code(case: &str, key: &str) -> AnovaCode {
    match case {
        "outcome_constant" => AnovaCode::OutcomeConstant,
        "all_groups_constant" => AnovaCode::WithinVarianceZero,
        "some_constant" if key.starts_with("welch") => AnovaCode::WelchVarianceZero,
        "some_constant" if key.starts_with("gh.") => AnovaCode::PairBothConstant,
        "n2" if key.starts_with("levene") => AnovaCode::LeveneDeviationsEqual,
        _ => panic!("{case}: {key} is NC in the fixture, and no code is expected for it"),
    }
}

#[test]
fn the_one_way_anova_matches_r_within_the_gates() {
    let expected = expected();
    let mut worst: BTreeMap<&'static str, (f64, String)> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut compared = 0usize;
    for case in cases() {
        let want = &expected[&case.name];
        let d = data(&case);
        let params = AnovaParams {
            outcome: alias(&d, "y"),
            group: alias(&d, "g"),
            levels: case.include.clone(),
            post_hoc: post_hoc_for(&case.name),
            missing_codes: case.missing_codes.clone(),
        };
        let report = validate(&d, &spec(OperationId(1), &d, params.clone()));
        let result = compute(&d, &params);
        if let Some(refusal) = want.get("refused") {
            let e = result.expect_err("a refused case");
            let [code, index, count]: [&str; 3] =
                refusal.split(':').collect::<Vec<_>>().try_into().unwrap();
            assert_eq!(code, "group_too_small", "{}", case.name);
            assert_eq!(e.code, AnovaCode::GroupTooSmall, "{}", case.name);
            let int = |k| match e.params.get(&k) {
                Some(gaply_core::contracts::ParamValue::Int(v)) => *v,
                other => panic!("{k:?}: {other:?}"),
            };
            assert_eq!(
                int(gaply_core::contracts::ParamKey::Index).to_string(),
                index
            );
            assert_eq!(
                int(gaply_core::contracts::ParamKey::Count).to_string(),
                count
            );
            assert!(
                !report.errors.is_empty(),
                "{}: validate refuses too",
                case.name
            );
            continue;
        }
        let r = result.unwrap_or_else(|e| panic!("{}: refused {e:?}", case.name));
        assert!(
            report.errors.is_empty(),
            "{}: {:?}",
            case.name,
            report.errors
        );
        let got = keys(&r);
        for (key, value) in want {
            if key.starts_with("excluded.") && value == "0" {
                assert!(
                    got.get(key).is_none(),
                    "{}: {key} listed with no row",
                    case.name
                );
                continue;
            }
            if key == "posthoc" {
                assert_eq!(value, "NC");
                for t in &r.post_hoc {
                    assert!(
                        matches!(t.pairs, PostHocPairs::NotComputable(c) if c == nc_code(&case.name, key)),
                        "{}: post hoc {:?}",
                        case.name,
                        t.pairs
                    );
                }
                continue;
            }
            let Some(g) = got.get(key) else {
                let family_ran = match key.split('.').next().unwrap() {
                    "gh" => post_hoc_for(&case.name).contains(&PostHoc::GamesHowell),
                    "tukey" => post_hoc_for(&case.name).contains(&PostHoc::Tukey),
                    "bonf" => post_hoc_for(&case.name).contains(&PostHoc::Bonferroni),
                    _ => true,
                };
                assert!(!family_ran, "{}: no {key} in the result", case.name);
                continue;
            };
            match (value.as_str(), g) {
                ("NC", Got::Nc(c)) => {
                    assert_eq!(*c, nc_code(&case.name, key), "{}: {key}", case.name)
                }
                ("NC", _) => {
                    failures.push(format!("{}: {key} should be not computable", case.name))
                }
                ("NA", Got::Number(v)) => assert!(v.is_finite(), "{}: {key}", case.name),
                ("NA", Got::Below) => {}
                (_, Got::Nc(c)) => failures.push(format!(
                    "{}: {key} not computable ({c:?}), R {value}",
                    case.name
                )),
                (_, Got::Below) => {
                    let w: f64 = value.parse().unwrap();
                    assert!(w < 1e-9, "{}: {key} below the floor here, R {w}", case.name);
                }
                (_, Got::Number(v)) => {
                    let w: f64 = value.parse().unwrap();
                    // The 1e9 offset has its own gate, except where R's
                    // ptukey and qtukey decide the error.
                    let fam = match family(key) {
                        f @ ("tukey.p" | "gh.p" | "tukey.ci" | "gh.ci") => f,
                        _ if case.name == "ties_offset" => "offset",
                        f => f,
                    };
                    let rel = error(fam, *v, w);
                    compared += 1;
                    let e = worst.entry(fam).or_insert((0.0, String::new()));
                    if rel > e.0 {
                        *e = (rel, format!("{} {key}: gaply {v:e} R {w:e}", case.name));
                    }
                }
            }
        }
    }
    println!("compared {compared} numbers");
    for (fam, (rel, at)) in &worst {
        let gate = GATES
            .iter()
            .find(|(f, _)| f == fam)
            .map(|(_, g)| *g)
            .unwrap_or(0.0);
        println!("{fam:>10}: largest {rel:.3e} (gate {gate:e}) at {at}");
        if *rel > gate {
            failures.push(format!("{fam}: {rel:e} over its gate {gate:e} at {at}"));
        }
    }
    assert!(
        compared > 5_000,
        "only {compared} numbers compared: the fixture did not load"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
