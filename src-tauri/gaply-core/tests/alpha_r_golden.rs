//! **`gaply_core::stats_alpha` against R 4.6.1 and psych (Research OS,
//! Phase 8I, decisions 367 to 370), over `tests/fixtures/stats_alpha/`
//! (written by `/usr/local/bin/Rscript make_alpha_fixtures.R`).**
//!
//! * Every statistic against base R's formulas (the same formula written
//!   twice, the generator's header says so), each family's gate ten times
//!   the largest error measured over every row (decision 370), printed with
//!   the measurement:
//!
//!   ```text
//!   cargo test -p gaply_core --test alpha_r_golden -- --nocapture
//!   ```
//!
//! * alpha, standardized alpha, r̄, each item's corrected item-total
//!   correlation and alpha if left out, and Feldt's interval (where df2 ≤
//!   4e5) against psych::alpha, an independent implementation, on the same
//!   rows with the items reversed by the generator;
//! * `stats_dist::f_quantile_upper` against an R `uniroot` oracle on
//!   `pf(lower.tail = FALSE)`, in relative error in F, on a grid of its own
//!   to df1 99,999 and df2 1e7; `f_upper`'s p at the oracle's F is measured
//!   and printed, never gated (decision 368).
//!
//! The gates are asserted on every platform (decision 277).

use std::collections::BTreeMap;

use gaply_core::contracts::codes::Closed;
use gaply_core::stats_alpha::{compute, AlphaOutcome, AlphaParams, Interval, ScaleEnds, Stat};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_dist::{f_quantile_upper, f_upper};
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_alpha");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_alpha_fixtures.R";
const R_VERSION: &str = "R version 4.6.1";

/// The gates against base R per family, each ten times the largest error
/// measured over every case (decision 370), rounded up at one significant
/// figure; relative, absolute where R's value is 0 or a correlation is
/// within 1e-9 of ±1 (`r.near1`, ±1 exactly on both sides by decision 352's
/// test). Measured 1 Nov 2026, macOS aarch64, over 712 values of 17 cases
/// (15 computed, 2 refused).
const GATES: &[(&str, f64)] = &[
    ("alpha", 2e-12),           // 1.568e-13, big alpha
    ("ci", 5e-12),              // 4.287e-13, big ci_upper
    ("std_alpha", 3e-14),       // 2.725e-15, totals_constant
    ("r", 3e-15),               // 2.205e-16, others_constant r_max
    ("r.near1", 0.0),           // ABSOLUTE 0, every ±1 on both sides
    ("item.mean", 3e-13),       // 2.116e-14, big item.1.mean
    ("item.sd", 2e-13),         // 1.857e-14, big item.1.sd
    ("item.scale_mean", 3e-13), // 2.199e-14, big item.0.scale_mean
    ("item.scale_var", 2e-13),  // 1.874e-14, big item.4.scale_var
    ("item.r_drop", 4e-15),     // 3.687e-16, k20_n200 item.1.r_drop
    ("item.alpha_drop", 4e-13), // 3.300e-14, big item.4.alpha_drop
];

/// The gates against psych, measured the same way (211 values, 1 Nov 2026,
/// psych 2.6.9). psych's worst are all on the 1e9 offset, where it loses
/// about 1e-9 that base R and Gaply do not (they agree there to 1e-13);
/// `r.near1` is a correlation psych gives as −1 + 1 ulp and Gaply as −1.
const PSYCH_GATES: &[(&str, f64)] = &[
    ("alpha", 3e-8),           // 2.066e-9, offset raw_alpha
    ("ci", 6e-8),              // 5.268e-9, offset ci_lower
    ("std_alpha", 5e-14),      // 4.672e-15, totals_constant
    ("r", 7e-15),              // 6.239e-16, likert_unreversed r_mean
    ("r.near1", 3e-15),        // 2.220e-16, totals_constant item.0.r_drop
    ("item.r_drop", 5e-8),     // 4.443e-9, offset item.2.r_drop
    ("item.alpha_drop", 7e-8), // 6.636e-9, offset item.1.alpha_drop
];

/// The cases psych::alpha refuses with an error even with `delete = FALSE`
/// (measured 1 Nov 2026, psych 2.6.9): an item constant over the rows. The
/// base-R formulas stand alone there.
const PSYCH_REFUSES: &[&str] = &["constant_item"];

/// `f_quantile_upper` against the uniroot oracle, relative error in F, ten
/// times the worst measured (1 Nov 2026, macOS aarch64, 213 rows), in two
/// regions:
///
/// * where Feldt's interval reaches, df2 = df1 · (k − 1) with k at most 128
///   (df2 ≤ 127 · df1): worst 1.107e-10 at p 0.5, df1 99,999, df2 1e7; at
///   Feldt's two points (p 0.025 and 0.975) 2.99e-11 at df1 99,999, df2 1e7,
///   and 3.74e-12 at df2 4,899,951;
/// * elsewhere on the grid (a small df1 with a large df2, which no alpha
///   reaches): worst 1.887e-8 at p 0.5, df1 2, df2 1e7, where `f_upper`'s
///   own p is 1.99e-8 from the oracle's (at p 0.025, df1 9, df2 1e7):
///   statrs's incomplete beta, which the bisection inherits.
const F_QUANTILE_GATE: f64 = 2e-9;
const F_QUANTILE_GATE_ELSEWHERE: f64 = 2e-7;

/// A fixture's rows, split on tabs, after its `#` header (which must name
/// the generator and R's version) and its column header.
fn table(name: &str, header: &str) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(format!("{DIR}/{name}")).expect("fixture");
    let mut lines = text.lines();
    let first = lines.next().unwrap();
    assert!(first.contains(PROVENANCE), "{name}: {first}");
    assert!(lines.next().unwrap().contains(R_VERSION), "{name}");
    let head = lines.next().unwrap();
    assert!(head.starts_with(header), "{name}: {head}");
    lines
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

struct Case {
    name: String,
    file: String,
    items: Vec<String>,
    reversed: Vec<String>,
    scale: Option<ScaleEnds>,
    codes: Vec<f64>,
}

fn words(s: &str) -> Vec<String> {
    if s == "-" {
        Vec::new()
    } else {
        s.split(' ').map(str::to_string).collect()
    }
}

fn cases() -> Vec<Case> {
    table(
        "cases.tsv",
        "case\tfile\titems\treversed\tscale\tmissing_codes",
    )
    .into_iter()
    .map(|f| Case {
        name: f[0].clone(),
        file: f[1].clone(),
        items: words(&f[2]),
        reversed: words(&f[3]),
        scale: (f[4] != "-").then(|| {
            let v: Vec<f64> = f[4].split(' ').map(|x| x.parse().unwrap()).collect();
            ScaleEnds {
                min: v[0],
                max: v[1],
            }
        }),
        codes: words(&f[5]).iter().map(|v| v.parse().unwrap()).collect(),
    })
    .collect()
}

fn blocks(name: &str, header: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for f in table(name, header) {
        out.entry(f[0].clone())
            .or_default()
            .insert(f[1].clone(), f[2].clone());
    }
    out
}

fn dataset_from(headers: Vec<String>, rows: Vec<Vec<String>>, bytes: &[u8]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(bytes, "alpha golden", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// The 100,000-row case, from the generator's formula: the same IEEE
/// operations, so the same doubles, written shortest-round-trip.
fn big() -> Dataset {
    let rows = (0u64..100_000)
        .map(|i| {
            let base = ((i * 104_729) % 1009) as f64 / 50.0;
            let mut row = vec![(i + 1).to_string()];
            for j in 1..=5u64 {
                let v = ((i * 7919 * j) % 10_007) as f64 / 100.0 + base;
                row.push(format!("{v:?}"));
            }
            row
        })
        .collect();
    let mut headers = vec!["id".to_string()];
    headers.extend((1..=5).map(|j| format!("b{j}")));
    dataset_from(headers, rows, b"big by formula")
}

fn data(case: &Case) -> Dataset {
    if case.file == "-" {
        assert_eq!(case.name, "big");
        return big();
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

fn params(case: &Case, d: &Dataset) -> AlphaParams {
    AlphaParams {
        items: case.items.iter().map(|h| alias(d, h)).collect(),
        reversed: case.reversed.iter().map(|h| alias(d, h)).collect(),
        scale: case.scale,
        missing_codes: case.codes.clone(),
    }
}

/* ---- what a result says, by the fixture's keys ----------------------------- */

fn stat(s: Stat) -> String {
    match s {
        Stat::Value(v) => format!("{v:?}"),
        Stat::NotComputable(c) => format!("NC:{}", c.wire().trim_start_matches("stats.alpha.")),
    }
}

/// The result as the generator's keys.
fn keys(r: &AlphaOutcome) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut put = |k: String, v: String| {
        out.insert(k, v);
    };
    put("n".into(), r.rows_used.to_string());
    put("k".into(), r.k.to_string());
    put("alpha".into(), stat(r.alpha));
    match r.interval {
        Interval::Computed { lower, upper } => {
            put("ci_lower".into(), format!("{lower:?}"));
            put("ci_upper".into(), format!("{upper:?}"));
        }
        Interval::NotComputable(c) => {
            let w = format!("NC:{}", c.wire().trim_start_matches("stats.alpha."));
            put("ci_lower".into(), w.clone());
            put("ci_upper".into(), w);
        }
    }
    put("std_alpha".into(), stat(r.standardized));
    put("r_mean".into(), stat(r.inter_item.mean));
    put("r_min".into(), stat(r.inter_item.min));
    put("r_max".into(), stat(r.inter_item.max));
    for n in &r.notes {
        put(
            format!("note.{}", n.wire().trim_start_matches("stats.alpha.")),
            "1".into(),
        );
    }
    for (i, it) in r.items.iter().enumerate() {
        let p = format!("item.{i}.");
        put(format!("{p}reversed"), u8::from(it.reversed).to_string());
        put(format!("{p}mean"), stat(it.mean));
        put(format!("{p}sd"), stat(it.sd));
        put(format!("{p}scale_mean"), stat(it.scale_mean_if_deleted));
        put(format!("{p}scale_var"), stat(it.scale_variance_if_deleted));
        put(format!("{p}r_drop"), stat(it.corrected_item_total));
        put(format!("{p}alpha_drop"), stat(it.alpha_if_deleted));
    }
    for e in &r.exclusions {
        let col = match e.params.values().next() {
            Some(gaply_core::contracts::ParamValue::Int(c)) => *c,
            other => panic!("exclusion column {other:?}"),
        };
        let index = r
            .items
            .iter()
            .position(|it| i64::from(it.item.0 .0) == col)
            .expect("an item");
        put(
            format!(
                "excl.{}.{index}",
                e.code.wire().trim_start_matches("stats.alpha.")
            ),
            e.count.to_string(),
        );
    }
    out
}

/// A key's gate family; `None` for an exact key (counts, flags, words).
fn family(key: &str, want: f64) -> Option<&'static str> {
    let tail = key.rsplit('.').next().unwrap_or("");
    let near1 = want.abs() > 1.0 - 1e-9;
    Some(match key {
        "n" | "k" => return None,
        "alpha" | "raw_alpha" => "alpha",
        "ci_lower" | "ci_upper" => "ci",
        "std_alpha" => "std_alpha",
        "r_mean" | "r_min" | "r_max" if near1 => "r.near1",
        "r_mean" | "r_min" | "r_max" => "r",
        _ if key.starts_with("excl.") || key.starts_with("note.") || tail == "reversed" => {
            return None
        }
        _ => match tail {
            "r_drop" if near1 => "r.near1",
            "mean" => "item.mean",
            "sd" => "item.sd",
            "scale_mean" => "item.scale_mean",
            "scale_var" => "item.scale_var",
            "r_drop" => "item.r_drop",
            "alpha_drop" => "item.alpha_drop",
            other => panic!("a key of no family: {key} ({other})"),
        },
    })
}

fn gate(gates: &[(&str, f64)], fam: &str) -> f64 {
    gates
        .iter()
        .find(|g| g.0 == fam)
        .unwrap_or_else(|| panic!("no gate {fam}"))
        .1
}

fn error(fam: &str, got: f64, want: f64) -> f64 {
    if fam == "r.near1" || want == 0.0 {
        (got - want).abs()
    } else {
        (got - want).abs() / want.abs()
    }
}

/// Compares `want` (one case's block) with `got`'s keys: words exactly, and
/// numbers within their family's gate; the worst per family into `worst`.
fn compare(
    case: &str,
    want: &BTreeMap<String, String>,
    got: &BTreeMap<String, String>,
    gates: &[(&str, f64)],
    worst: &mut BTreeMap<&'static str, (f64, String)>,
    failures: &mut Vec<String>,
) -> usize {
    let mut compared = 0;
    for (key, w) in want {
        let key_in_got = if key == "raw_alpha" {
            "alpha"
        } else {
            key.as_str()
        };
        let Some(g) = got.get(key_in_got) else {
            failures.push(format!("{case} {key}: R has it, Gaply does not"));
            continue;
        };
        compared += 1;
        let (Ok(wv), Ok(gv)) = (w.parse::<f64>(), g.parse::<f64>()) else {
            if w != g {
                failures.push(format!("{case} {key}: R {w}, Gaply {g}"));
            }
            continue;
        };
        match family(key, wv) {
            None => {
                if wv != gv {
                    failures.push(format!("{case} {key}: R {w}, Gaply {g}"));
                }
            }
            Some(fam) => {
                let e = error(fam, gv, wv);
                let entry = worst.entry(fam).or_insert((0.0, String::new()));
                if e > entry.0 {
                    *entry = (e, format!("{case} {key}"));
                }
                if e > gate(gates, fam) {
                    failures.push(format!(
                        "{case} {key}: R {w}, Gaply {g}, error {e:.3e} ({fam})"
                    ));
                }
            }
        }
    }
    compared
}

fn print_worst(title: &str, worst: &BTreeMap<&'static str, (f64, String)>, gates: &[(&str, f64)]) {
    println!("{title}");
    for (fam, (e, at)) in worst {
        println!(
            "  {fam:<16} worst {e:.3e} at {at}, gate {:.0e}",
            gate(gates, fam)
        );
    }
}

#[test]
fn every_case_matches_r_within_its_family_gate() {
    let want = blocks("expected.tsv", "case\tkey\tvalue");
    let cases = cases();
    assert_eq!(cases.len(), want.len(), "one expected block per case");
    let mut worst = BTreeMap::new();
    let mut failures = Vec::new();
    let mut compared = 0usize;
    for case in &cases {
        let w = &want[&case.name];
        let d = data(case);
        let got = compute(&d, &params(case, &d));
        if let Some(code) = w.get("refused") {
            match got {
                Err(e) => assert_eq!(
                    e.code.wire(),
                    format!("stats.alpha.{code}"),
                    "{}",
                    case.name
                ),
                Ok(_) => {
                    failures.push(format!("{}: R refuses ({code}), Gaply computed", case.name))
                }
            }
            compared += 1;
            continue;
        }
        let r = got.unwrap_or_else(|e| panic!("{}: refused {:?}", case.name, e.code));
        let g = keys(&r);
        for k in g.keys() {
            if !w.contains_key(k) {
                failures.push(format!(
                    "{} {k}: Gaply has it ({}), R does not",
                    case.name, g[k]
                ));
            }
        }
        compared += compare(&case.name, w, &g, GATES, &mut worst, &mut failures);
    }
    print_worst(
        &format!("base R: {compared} values over {} cases", cases.len()),
        &worst,
        GATES,
    );
    assert!(compared > 600, "{compared} values compared");
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn psych_agrees_within_its_gates() {
    let want = blocks("psych.tsv", "case\tkey\tvalue");
    let mut worst = BTreeMap::new();
    let mut failures = Vec::new();
    let mut compared = 0usize;
    let cases = cases();
    for case in &cases {
        let Some(w) = want.get(&case.name) else {
            continue;
        };
        assert_eq!(
            w.contains_key("error"),
            PSYCH_REFUSES.contains(&case.name.as_str()),
            "{}: psych's refusal is not the one recorded",
            case.name
        );
        if w.contains_key("error") {
            continue;
        }
        let d = data(case);
        let r = compute(&d, &params(case, &d)).expect("computed");
        let g = keys(&r);
        // psych reports a statistic Gaply calls not computable as NaN or a
        // number; those keys are compared only where Gaply has a number.
        let w: BTreeMap<String, String> = w
            .iter()
            .filter(|(k, _)| {
                let k = if k.as_str() == "raw_alpha" {
                    "alpha"
                } else {
                    k.as_str()
                };
                g.get(k).is_some_and(|v| !v.starts_with("NC:"))
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        compared += compare(&case.name, &w, &g, PSYCH_GATES, &mut worst, &mut failures);
    }
    print_worst(&format!("psych: {compared} values"), &worst, PSYCH_GATES);
    assert!(compared > 150, "{compared} values compared");
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn f_quantile_upper_matches_the_uniroot_oracle() {
    let rows = table("f_quantile.tsv", "p_upper\tdf1\tdf2\tf");
    assert!(rows.len() > 200, "{} rows", rows.len());
    let (mut worst, mut worst_p) = ((0.0f64, String::new()), (0.0f64, String::new()));
    let mut failures = Vec::new();
    for f in &rows {
        let [p, d1, d2, want] = [0, 1, 2, 3].map(|i| f[i].parse::<f64>().unwrap());
        let at = format!("p {p} df1 {d1} df2 {d2}");
        match f_quantile_upper(p, d1, d2) {
            None => failures.push(format!("{at}: None")),
            Some(got) => {
                let e = (got - want).abs() / want;
                if e > worst.0 {
                    worst = (e, at.clone());
                }
                let gate = if d2 <= 127.0 * d1 {
                    F_QUANTILE_GATE
                } else {
                    F_QUANTILE_GATE_ELSEWHERE
                };
                if e > gate {
                    failures.push(format!(
                        "{at}: oracle {want:?}, Gaply {got:?}, relative {e:.3e}"
                    ));
                }
            }
        }
        // Measured, never gated: f_upper's p at the oracle's F.
        if let Some(q) = f_upper(want, d1, d2) {
            let e = (q - p).abs() / p;
            if e > worst_p.0 {
                worst_p = (e, at);
            }
        }
    }
    println!(
        "f_quantile_upper over {} rows: worst relative error in F {:.3e} at {}, gates {:.0e} (df2 <= 127 df1) and {:.0e}",
        rows.len(),
        worst.0,
        worst.1,
        F_QUANTILE_GATE,
        F_QUANTILE_GATE_ELSEWHERE
    );
    println!(
        "f_upper's p at the oracle's F: worst relative error {:.3e} at {}",
        worst_p.0, worst_p.1
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
