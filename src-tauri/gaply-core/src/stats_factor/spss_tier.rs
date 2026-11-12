//! stats_factor's rotations against SPSS's own output (Research OS, Phase 9B-2,
//! decision 719): SPSS 23's and 27's solutions for EFAtools 1.1.0's four
//! simulated test models, eight in all, as
//! `tests/fixtures/stats_factor/make_factor_spss_fixtures.R` writes them from
//! EFAtools' data (Grieder and Steiner, Behavior Research Methods, 2022). SPSS's
//! own unrotated principal axis loadings are rotated by this module's varimax and
//! promax, and compared with SPSS's varimax loadings, promax pattern and factor
//! correlations; the cycles must equal the count that reproduces SPSS's varimax
//! (decision 718).
//!
//! A unit test inside the module rather than a file under `tests/`: `varimax` and
//! `promax_of` are private. It reads its fixture with `include_str!`, so the
//! module's purity scan (`tests/contracts_are_pure.rs`) finds no filesystem here;
//! and since that scan forbids printing, the measured errors are reported in the
//! assertion's message, which a run with the gates at 0 shows in full.
//!
//! EFAtools' other nine solutions come from real studies and are not committed;
//! they were checked in the session that wrote this file (12 Nov 2026) by
//! pointing `SOLUTIONS` at an uncommitted file of all 17 (decision 719): every
//! cycle count equal, worst errors 2.526e-15 (varimax), 2.776e-15 (pattern) and
//! 3.136e-15 (factor correlations), all WJIV_3_5 in SPSS 23
//! (~/gaply-corpus/os/phase9/9b2/logs/p2_spss_tier_all17_1794457843.log).

use std::collections::BTreeMap;
use std::convert::Infallible;

use super::*;

const SOLUTIONS: &str = include_str!("../../tests/fixtures/stats_factor/spss_solutions.tsv");

/// Ten times the largest error measured over the eight solutions, rounded up
/// (12 Nov 2026, macOS aarch64, a run with the gates at 0: varimax 1.527e-15,
/// pattern 1.887e-15, factor correlations 1.665e-15;
/// ~/gaply-corpus/os/phase9/9b2/logs/p2_spss_tier_measure_1794457801.log).
const VARIMAX_GATE: f64 = 1.6e-14;
const PATTERN_GATE: f64 = 1.9e-14;
const PHI_GATE: f64 = 1.7e-14;

type Matrix = Vec<Vec<f64>>;

/// Each solution's matrices by key, and its cycles.
fn solutions() -> BTreeMap<String, (BTreeMap<String, Matrix>, u64)> {
    let mut out: BTreeMap<String, (BTreeMap<String, Matrix>, u64)> = BTreeMap::new();
    let mut lines = SOLUTIONS.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some("solution\tkey\ti\tj\tvalue"));
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        let entry = out.entry(f[0].to_string()).or_default();
        if f[1] == "cycles" {
            entry.1 = f[4].parse().unwrap();
            continue;
        }
        let (i, j): (usize, usize) = (f[2].parse().unwrap(), f[3].parse().unwrap());
        let m = entry.0.entry(f[1].to_string()).or_default();
        while m.len() <= i {
            m.push(Vec::new());
        }
        while m[i].len() <= j {
            m[i].push(f64::NAN);
        }
        m[i][j] = f[4].parse().unwrap();
    }
    out
}

/// The largest |got − want|. The shapes must be equal and no error NaN, so a
/// matrix with a row or a column too few, or a NaN on either side, cannot pass on
/// the overlap (the review of 9B-2, finding 6: a Φ cut to (m − 1) × (m − 1)
/// passed before).
fn worst(got: &[Vec<Stat>], want: &Matrix) -> f64 {
    assert_eq!(got.len(), want.len(), "rows");
    let mut out = 0.0f64;
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.len(), w.len(), "columns");
        for (s, x) in g.iter().zip(w) {
            let e = match s {
                Stat::Value(v) => (v - x).abs(),
                Stat::NotComputable(_) => f64::INFINITY,
            };
            assert!(!e.is_nan(), "a NaN, Gaply {s:?} against {x}");
            out = out.max(e);
        }
    }
    out
}

#[test]
fn varimax_and_promax_give_spss_s_own_solutions() {
    let all = solutions();
    assert!(all.len() >= 8, "{} solutions", all.len());
    let (mut v_worst, mut p_worst, mut phi_worst) = ((0.0f64, String::new()), (0.0f64, String::new()), (0.0f64, String::new()));
    for (name, (m, cycles)) in &all {
        let unrotated = &m["paf_load"];
        let v = varimax::<Infallible>(unrotated, 1000, || Ok(())).ok().flatten().unwrap_or_else(|| panic!("{name}: no varimax"));
        assert_eq!(v.cycles, *cycles, "{name}: the cycles that reproduce SPSS's varimax");
        let rotated = varimax_result(Varimax { loadings: v.loadings.clone(), t: v.t.clone(), cycles: v.cycles }, unrotated.len());
        let e = worst(&rotated.loadings, &m["var_load"]);
        if e > v_worst.0 {
            v_worst = (e, name.clone());
        }
        let promax = promax_of(&v.loadings, v.cycles).unwrap_or_else(|| panic!("{name}: no promax"));
        let e = worst(&promax.loadings, &m["pro_load"]);
        if e > p_worst.0 {
            p_worst = (e, name.clone());
        }
        let e = worst(promax.factor_correlations.as_ref().unwrap(), &m["pro_phi"]);
        if e > phi_worst.0 {
            phi_worst = (e, name.clone());
        }
    }
    let report = format!(
        "{} solutions: varimax {:.3e} ({}), promax pattern {:.3e} ({}), factor correlations {:.3e} ({})",
        all.len(),
        v_worst.0,
        v_worst.1,
        p_worst.0,
        p_worst.1,
        phi_worst.0,
        phi_worst.1
    );
    assert!(v_worst.0 <= VARIMAX_GATE && p_worst.0 <= PATTERN_GATE && phi_worst.0 <= PHI_GATE, "{report}");
}
