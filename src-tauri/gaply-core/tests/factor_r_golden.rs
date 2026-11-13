//! **`gaply_core::stats_factor` against R (Research OS, Phase 9B-1, decision
//! 701; the rotations as SPSS computes them since 9B-2, decision 719).** Every
//! number of every case of `tests/fixtures/stats_factor/make_factor_fixtures.R`,
//! written by R 4.6.1 by a different route from Rust's (solve for the inverse,
//! R's own principal axis loop checked against EFAtools' .PAF, and EFAtools
//! 1.1.0's SPSS varimax and promax, themselves checked against a second route in
//! base R written from IBM's document), with decision 699's sign and Cholesky
//! rules applied to R's results. Each family's gate is ten times its measured
//! error, tiered by the correlation matrix's condition number; every refusal is
//! checked by its code and its parameter.
//!
//! ```text
//! cargo test -p gaply_core --test factor_r_golden -- --nocapture
//! ```

use std::collections::BTreeMap;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{ParamKey, ParamValue};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_factor::{compute, Extraction, FactorOutcome, FactorParams, Rotation, Stat};
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_factor");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_factor_fixtures.R";

/// Each family's gate in each tier of the condition number (≤ 1e3, ≤ 1e8,
/// > 1e8): ten times the largest error measured in it, rounded up (macOS
/// aarch64, 11 Nov 2026, ~/gaply-corpus/os/phase9/9b1/logs/p2_golden_measure_1794411871.log, a run
/// with every gate at 0); `None` where no case falls. Measured:
///
/// | family | ≤ 1e3 | ≤ 1e8 | > 1e8 |
/// |---|---|---|---|
/// | descriptive | 0 (exact) | 0 (exact) | 1.988e-15 |
/// | determinant | 4.894e-15 | 5.699e-10 | none |
/// | kmo | 1.332e-15 | 6.788e-11 | none |
/// | bartlett | 1.339e-13 | 3.857e-11 | none |
/// | eigenvalue | 5.662e-15 | 9.564e-14 | 5.254e-14 |
/// | communality | 1.465e-14 | 6.006e-14 | 2.109e-15 |
/// | extraction_sums | 3.007e-14 | 2.970e-15 | 1.237e-15 |
/// | loading | 4.596e-14 | 2.907e-15 | 1.554e-15 |
/// | rotation | 6.717e-14 | 3.164e-15 | none |
/// | structure | 3.385e-14 | none | none |
/// | phi | 2.530e-14 | none | none |
///
/// The rotation families are against EFAtools 1.1.0's SPSS route since 9B-2
/// (decision 719), measured 12 Nov 2026 with those gates at 0
/// (~/gaply-corpus/os/phase9/9b2/logs/p2_golden_measure_1794457651.log); the
/// other rows are 9B-1's, measured again there unchanged. `rot_iterations`,
/// varimax's cycles, is compared exactly.
///
/// 9B-1's bug hunt (decision 701, seed 20260701) measured a standard deviation
/// one ulp from R's (2.044e-16; R's `sd` and `stats_verify::sample_variance`
/// round apart), and the descriptive gate is ten times that.
///
/// 9B-2's hunt (decision 719, 240 seeded datasets, seed 20260719,
/// ~/gaply-corpus/os/phase9/9b2/hunt/) found two numbers above these gates (three
/// failure lines: one Φ value at [0, 6] and its symmetric twin), each shown to be
/// rounding and left as the hunt's own record, not raised here:
/// - h215's factor correlations, 2.783e-13 against 2.6e-13 (nine principal axis
///   factors from eleven items). Gaply's promax on its own loadings gives
///   EFAtools' Φ within 7.494e-15; EFAtools' own Φ moves by 2.726e-13 between
///   Gaply's and R's loadings, which differ by 2.916e-14 (hunt_attribution log).
/// - h232's seventh eigenvalue's percent, 1.026e-13 against 5.7e-14 (seven items,
///   fifty rows, a Heywood-prone structure). R disagrees with itself by 9.226e-14
///   on the same matrix, `eigen` with vectors against values only (h232_modes
///   log); Gaply's value, in five item orders, lies within 4.0e-15 of R's
///   values-only one (h232_orders log). This module's eigenvalues are 9B-1's,
///   unchanged.
const GATES: &[(&str, [Option<f64>; 3])] = &[
    ("descriptive", [Some(2.1e-15), Some(2.1e-15), Some(2e-14)]),
    ("determinant", [Some(4.9e-14), Some(5.7e-9), None]),
    ("kmo", [Some(1.4e-14), Some(6.8e-10), None]),
    ("bartlett", [Some(1.4e-12), Some(3.9e-10), None]),
    ("eigenvalue", [Some(5.7e-14), Some(9.6e-13), Some(5.3e-13)]),
    ("communality", [Some(1.5e-13), Some(6.1e-13), Some(2.2e-14)]),
    ("extraction_sums", [Some(3.1e-13), Some(3e-14), Some(1.3e-14)]),
    ("loading", [Some(4.6e-13), Some(3e-14), Some(1.6e-14)]),
    ("rotation", [Some(6.8e-13), Some(3.2e-14), None]),
    ("structure", [Some(3.4e-13), None, None]),
    ("phi", [Some(2.6e-13), None, None]),
];

fn tier(condition: f64) -> usize {
    if condition <= 1e3 {
        0
    } else if condition <= 1e8 {
        1
    } else {
        2
    }
}

pub struct Case {
    pub name: String,
    pub data: String,
    pub extraction: Extraction,
    pub factors: Option<u32>,
    pub rotation: Rotation,
    pub iterations: u32,
    pub convergence: f64,
    pub codes: Vec<f64>,
    pub expect: String,
    pub condition: f64,
}

fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

pub fn cases() -> Vec<Case> {
    let text = read("cases.tsv");
    assert!(text.lines().any(|l| l.contains(PROVENANCE)), "cases.tsv: not written by {PROVENANCE}");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(
        lines.next(),
        Some("case\tdata\textraction\tfactors\trotation\titerations\tconvergence\tcodes\texpect\tcondition\tnote")
    );
    lines
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            Case {
                name: f[0].into(),
                data: f[1].into(),
                extraction: if f[2] == "principal_axis" { Extraction::PrincipalAxis } else { Extraction::PrincipalComponents },
                factors: if f[3] == "kaiser" { None } else { Some(f[3].parse().unwrap()) },
                rotation: match f[4] {
                    "varimax" => Rotation::Varimax,
                    "promax" => Rotation::Promax,
                    _ => Rotation::None,
                },
                iterations: f[5].parse().unwrap(),
                convergence: f[6].parse().unwrap(),
                codes: f[7].split(';').filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect(),
                expect: f[8].into(),
                condition: f[9].parse().unwrap_or(f64::NAN),
            }
        })
        .collect()
}

/// make_factor_fixtures.R's formula_data, in the same integer arithmetic.
pub fn formula_rows(n: u64, p: u64, k: u64) -> Vec<Vec<String>> {
    (1..=n)
        .map(|i| {
            (1..=p)
                .map(|j| {
                    let f = (j - 1) % k;
                    let score = ((i * 7907 + f * 15485863) % 1013) as i64 - 506;
                    let noise = ((i * 104729 + j * 7919 + i * j * 31) % 997) as i64 - 498;
                    (2 * score + noise).to_string()
                })
                .collect()
        })
        .collect()
}

pub fn dataset_of(data: &str) -> (Dataset, usize) {
    let (headers, rows): (Vec<String>, Vec<Vec<String>>) = if let Some(spec) = data.strip_prefix("formula:") {
        let v: Vec<u64> = spec.split(':').map(|s| s.parse().unwrap()).collect();
        ((1..=v[1]).map(|j| format!("x{j}")).collect(), formula_rows(v[0], v[1], v[2]))
    } else {
        let text = read(data);
        let mut lines = text.lines();
        let headers = lines.next().unwrap().split(',').map(String::from).collect();
        (headers, lines.map(|l| l.split(',').map(String::from).collect()).collect())
    };
    let p = headers.len();
    let d = build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(data.as_bytes(), "factor golden", TableRef::FirstTable),
    )
    .expect("dataset");
    (d, p)
}

pub fn params_of(case: &Case, p: usize) -> FactorParams {
    let mut items: Vec<Alias> = (1..=p as u32).map(|j| Alias(ColumnAlias(j))).collect();
    if case.name == "items_out_of_order" {
        items.swap(0, 1);
    }
    FactorParams {
        items,
        extraction: case.extraction,
        factors: case.factors,
        rotation: case.rotation,
        iterations: case.iterations,
        convergence: case.convergence,
        missing_codes: case.codes.clone(),
    }
}

/// expected.tsv: case → [(key, i, j, value text)].
fn expected() -> BTreeMap<String, Vec<(String, usize, usize, String)>> {
    let text = read("expected.tsv");
    let mut out: BTreeMap<String, Vec<(String, usize, usize, String)>> = BTreeMap::new();
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some("case\tkey\ti\tj\tvalue"));
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        out.entry(f[0].into()).or_default().push((f[1].into(), f[2].parse().unwrap(), f[3].parse().unwrap(), f[4].into()));
    }
    out
}

/// The statistic Gaply gives for a key at (i, j), or `None` where it gives none.
fn got(r: &FactorOutcome, key: &str, i: usize, j: usize) -> Option<Stat> {
    let rot = r.rotation.as_ref();
    Some(match key {
        "mean" => r.descriptives[i].mean,
        "sd" => r.descriptives[i].sd,
        "det" => r.determinant.value,
        "logdet" => r.determinant.log,
        "kmo" => r.kmo.overall,
        "msa" => r.kmo.items[i].msa,
        "bartlett_chi" => r.bartlett.chi_square,
        "bartlett_p" => r.bartlett.p,
        "eval" => r.eigenvalues[j].total,
        "eval_pct" => r.eigenvalues[j].percent,
        "eval_cum" => r.eigenvalues[j].cumulative,
        "comm_init" => r.extraction.communalities[i].initial,
        "comm_ext" => r.extraction.communalities[i].extraction,
        "ext_ss" => r.extraction.sums[j].total,
        "ext_pct" => r.extraction.sums[j].percent,
        "ext_cum" => r.extraction.sums[j].cumulative,
        "loading" => r.extraction.loadings[i][j],
        "rot_loading" => rot?.loadings[i][j],
        "rot_t" => rot?.transformation.as_ref()?[i][j],
        "rot_ss" => rot?.sums.as_ref()?[j].total,
        "rot_pct" => rot?.sums.as_ref()?[j].percent,
        "rot_cum" => rot?.sums.as_ref()?[j].cumulative,
        "structure" => rot?.structure.as_ref()?[i][j],
        "phi" => rot?.factor_correlations.as_ref()?[i][j],
        _ => return None,
    })
}

fn family(key: &str) -> &'static str {
    match key {
        "mean" | "sd" => "descriptive",
        "det" | "logdet" => "determinant",
        "kmo" | "msa" => "kmo",
        "bartlett_chi" | "bartlett_p" => "bartlett",
        "eval" | "eval_pct" | "eval_cum" => "eigenvalue",
        "comm_init" | "comm_ext" => "communality",
        "ext_ss" | "ext_pct" | "ext_cum" => "extraction_sums",
        "loading" => "loading",
        "rot_loading" | "rot_t" | "rot_ss" | "rot_pct" | "rot_cum" => "rotation",
        "structure" => "structure",
        "phi" => "phi",
        other => panic!("key {other}"),
    }
}

/// The error of `got` against R's `want`: relative for a determinant and a p
/// (which may be far below 1), otherwise against max(1, |want|).
fn error(key: &str, got: f64, want: f64) -> f64 {
    let d = (got - want).abs();
    if matches!(key, "det" | "bartlett_p") {
        if want == 0.0 { d } else { d / want.abs() }
    } else {
        d / want.abs().max(1.0)
    }
}

#[test]
fn every_number_of_every_case_is_within_its_familys_gate_of_r() {
    let all = cases();
    let exp = expected();
    // −1 until a number of the tier is compared, so an exact 0 is told from an
    // empty tier.
    let none = || (-1.0f64, String::new());
    let mut worst: BTreeMap<&str, [(f64, String); 3]> = GATES.iter().map(|(f, _)| (*f, [none(), none(), none()])).collect();
    let (mut compared, mut refused, mut ok, mut skipped) = (0usize, 0usize, 0usize, 0usize);
    for case in &all {
        let (d, p) = dataset_of(&case.data);
        let result = compute(&d, &params_of(case, p));
        if case.expect != "ok" {
            refused += 1;
            let e = result.expect_err(&format!("{}: R refuses ({}), Gaply gave a result", case.name, case.expect));
            let (code, param) = case.expect.split_once(':').map_or((case.expect.as_str(), None), |(c, n)| (c, Some(n.parse::<i64>().unwrap())));
            assert_eq!(e.code.wire(), format!("stats.factor.{code}"), "{}", case.name);
            if let Some(n) = param {
                let key = if code == "extraction_not_converged" { ParamKey::Limit } else { ParamKey::Column };
                assert_eq!(e.params.get(&key), Some(&ParamValue::Int(n)), "{}: {:?}", case.name, e.params);
            }
            continue;
        }
        ok += 1;
        let r = result.unwrap_or_else(|e| panic!("{}: refused {}", case.name, e.code.wire()));
        let t = tier(case.condition);
        // Within a tie at the m-th eigenvalue the loadings are one choice of many
        // (the note says so): no number made from them is compared.
        let tied = exp[&case.name].iter().any(|(k, _, _, v)| k == "notes" && v.contains("eigenvalues_not_distinct"));
        for (key, i, j, value) in &exp[&case.name] {
            if tied && matches!(key.as_str(), "loading" | "comm_ext" | "rot_loading" | "rot_t" | "rot_ss" | "rot_pct" | "rot_cum" | "structure" | "phi") {
                skipped += 1;
                continue;
            }
            match key.as_str() {
                "rows_used" => assert_eq!(r.rows_used, value.parse::<u64>().unwrap(), "{}", case.name),
                "bartlett_df" => assert_eq!(r.bartlett.df, value.parse::<u64>().unwrap(), "{}", case.name),
                "m" => assert_eq!(r.extraction.factors, value.parse::<u64>().unwrap(), "{}", case.name),
                "iterations" => assert_eq!(r.extraction.iterations, value.parse::<u64>().unwrap(), "{}: the iterations used", case.name),
                "rot_iterations" => assert_eq!(
                    r.rotation.as_ref().map(|x| x.iterations),
                    Some(value.parse::<u64>().unwrap()),
                    "{}: varimax's cycles, exactly (decision 719)",
                    case.name
                ),
                "notes" => {
                    let mut got: Vec<&str> = r.notes.iter().map(|c| c.wire().trim_start_matches("stats.factor.")).collect();
                    got.sort();
                    let want: Vec<&str> = if value == "-" { vec![] } else { value.split(';').collect() };
                    assert_eq!(got, want, "{}: the notes", case.name);
                }
                _ => {
                    let g = got(&r, key, *i, *j).unwrap_or_else(|| panic!("{}: Gaply gives no {key} [{i}, {j}]", case.name));
                    compared += 1;
                    if let Some(code) = value.strip_prefix("NC:") {
                        assert_eq!(g, Stat::NotComputable(gaply_core::stats_factor::FactorCode::from_wire(&format!("stats.factor.{code}")).unwrap()), "{} {key}", case.name);
                        continue;
                    }
                    if value == "NaN" {
                        // R's NaN is a 0/0: a KMO or an MSA with no correlation is
                        // no_correlation (decision 714); any other key too_small.
                        let want = if matches!(key.as_str(), "kmo" | "msa") { "stats.factor.no_correlation" } else { "stats.factor.too_small" };
                        assert!(matches!(g, Stat::NotComputable(c) if c.wire() == want), "{} {key}: R's NaN, Gaply {g:?}, want {want}", case.name);
                        continue;
                    }
                    let want: f64 = value.parse().unwrap();
                    let Stat::Value(v) = g else { panic!("{} {key} [{i}, {j}]: {g:?}, R {want}", case.name) };
                    let e = error(key, v, want);
                    let w = &mut worst.get_mut(family(key)).unwrap()[t];
                    if e > w.0 {
                        *w = (e, format!("{} {key} [{i}, {j}]: gaply {v:e}, R {want:e}", case.name));
                    }
                }
            }
        }
        // Gaply gives nothing R does not: a rotation exactly where R wrote one.
        let r_rotated = exp[&case.name].iter().any(|(k, ..)| k == "rot_loading");
        assert_eq!(r.rotation.is_some(), r_rotated, "{}: a rotation", case.name);
    }
    println!("factor_r_golden: {} cases, {ok} with a result, {refused} refused, {compared} numbers compared, {skipped} skipped within a tie", all.len());
    let mut over = Vec::new();
    for (f, gates) in GATES {
        let w = &worst[f];
        println!("  {f:16} {:.3e} {:.3e} {:.3e}   ({} | {} | {})", w[0].0, w[1].0, w[2].0, w[0].1, w[1].1, w[2].1);
        for t in 0..3 {
            match gates[t] {
                Some(_) if w[t].0 < 0.0 => over.push(format!("{f}, tier {t}: a gate with no case")),
                Some(g) if w[t].0 > g => over.push(format!("{f}, tier {t}: {:e} over {g:e} ({})", w[t].0, w[t].1)),
                None if w[t].0 >= 0.0 => over.push(format!("{f}, tier {t}: a case where none was expected")),
                _ => {}
            }
        }
    }
    // 43 since decision 728's appended case (uncorrelated_item_pa_promax).
    assert_eq!(all.len(), 43, "make_factor_fixtures.R's cases");
    assert!(compared > 8000, "{compared} numbers compared");
    assert!(over.is_empty(), "over the gate:\n{}", over.join("\n"));
}

/* ------------------------------------------------------------------------ *
 *  The psych tier (decision 701): make_factor_psych_fixtures.R's
 *  psych_expected.tsv, psych 2.6.9's KMO, cortest.bartlett and fa(fm = "pa")
 *  unrotated, against Gaply at convergence 1e-10 and 1,000 iterations. Gates
 *  of its own. psych's rotations follow R's, not SPSS's, so since 9B-2 the
 *  tier holds none (decision 719).
 * ------------------------------------------------------------------------ */

/// Each family's gate: ten times the largest difference measured (macOS
/// aarch64, 11 Nov 2026), rounded up. psych's principal axis stops on the
/// change in the communalities' sum (min.err 1e-13), Gaply on the largest
/// change (1e-10), so the loadings differ by the two stopping points.
///
/// | family | measured | where |
/// |---|---|---|
/// | kmo (absolute) | 3.331e-16 | two_factor_6, an MSA |
/// | bartlett (relative) | 1.339e-13 | three_factor_12, p |
/// | communality (absolute) | 2.930e-10 | two_factor_6 |
/// | loading (absolute) | 2.149e-10 | two_factor_6 |
///
/// (~/gaply-corpus/os/phase9/9b1/logs/p2_psych_measure_1794412342.log, a run
/// with every gate at 0; these rows of psych_expected.tsv are byte for byte as
/// then, ~/gaply-corpus/os/phase9/9b2/fixtures_before/.)
const PSYCH_GATES: &[(&str, f64)] = &[("kmo", 3.4e-15), ("bartlett", 1.4e-12), ("communality", 3e-9), ("loading", 2.2e-9)];

#[test]
fn psych_agrees_within_its_own_gates() {
    let text = read("psych_expected.tsv");
    assert!(text.lines().any(|l| l.contains("/usr/local/bin/Rscript make_factor_psych_fixtures.R")));
    let mut rows: BTreeMap<String, Vec<(String, usize, usize, f64)>> = BTreeMap::new();
    for l in text.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        rows.entry(f[0].into()).or_default().push((f[1].into(), f[2].parse().unwrap(), f[3].parse().unwrap(), f[4].parse().unwrap()));
    }
    assert_eq!(rows.len(), 3, "three structures");
    let mut worst: BTreeMap<&str, (f64, String)> = PSYCH_GATES.iter().map(|(f, _)| (*f, (-1.0, String::new()))).collect();
    let mut see = |f: &'static str, e: f64, at: String| {
        let w = worst.get_mut(f).unwrap();
        if e > w.0 {
            *w = (e, at);
        }
    };
    for (case, list) in &rows {
        let (d, p) = dataset_of(&format!("data_{case}.csv"));
        let m = if case == "three_factor_12" { 3 } else { 2 };
        let run = |rotation| {
            let mut params = params_of(&Case {
                name: case.clone(),
                data: String::new(),
                extraction: Extraction::PrincipalAxis,
                factors: Some(m),
                rotation,
                iterations: 1000,
                convergence: 1e-10,
                codes: Vec::new(),
                expect: "ok".into(),
                condition: 1.0,
            }, p);
            params.iterations = 1000;
            compute(&d, &params).unwrap()
        };
        let none = run(Rotation::None);
        let val = |s: Stat| match s {
            Stat::Value(v) => v,
            other => panic!("{case}: {other:?}"),
        };
        for (key, i, j, want) in list {
            let at = format!("{case} {key} [{i}, {j}]");
            match key.as_str() {
                "kmo" => see("kmo", (val(none.kmo.overall) - want).abs(), at),
                "msa" => see("kmo", (val(none.kmo.items[*i].msa) - want).abs(), at),
                "bartlett_chi" => see("bartlett", (val(none.bartlett.chi_square) - want).abs() / want.abs(), at),
                "bartlett_p" => see("bartlett", (val(none.bartlett.p) - want).abs() / want.abs().max(f64::MIN_POSITIVE), at),
                "comm_ext" => see("communality", (val(none.extraction.communalities[*i].extraction) - want).abs(), at),
                "loading" => see("loading", (val(none.extraction.loadings[*i][*j]) - want).abs(), at),
                other => panic!("{other}: the psych tier holds no rotation since 9B-2"),
            }
        }
    }
    let mut over = Vec::new();
    for (f, gate) in PSYCH_GATES {
        let (e, at) = &worst[f];
        println!("  psych {f:22} {e:.3e}  ({at})");
        assert!(*e >= 0.0, "{f}: nothing compared");
        if e > gate {
            over.push(format!("{f}: {e:e} over {gate:e} ({at})"));
        }
    }
    assert!(over.is_empty(), "over the gate:\n{}", over.join("\n"));
}
