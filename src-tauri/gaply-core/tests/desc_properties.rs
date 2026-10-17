//! **Property tests of the descriptive summary (Research OS, Phase 2A).**
//!
//! proptest, approved by name as a dev-dependency (decisions.md, item 3),
//! with a fixed seed, failure persistence off (no proptest-regressions file
//! is ever written) and bounded generators: values in ±1e6, shifts c in
//! ±1e6, scales |k| in [1e-3, 1e3].
//!
//! THE ORACLE. Values are drawn on the grid j/128 (|j| ≤ 1.28e8), where every
//! value is exact in binary and so are its sums, so n·Σx and n·Σx² − (Σx)²
//! are exact integers (i128) and the oracle's mean and sd carry one to
//! three roundings. Counts, min, the quartiles and max must equal the
//! oracle's exactly (the quartiles by R's type-7 definition on the sorted
//! values); mean and sd must meet the golden gate, |gaply − oracle| ≤
//! 1e-12 · max(|oracle|, s), with s the oracle's sd, or 1 when it is 0 or
//! undefined.
//!
//! THE RELATIONS. mean(x + c) = mean(x) + c and sd(k·x) = |k|·sd(x) are
//! checked on inputs where x + c and k·x are themselves exact (c on the
//! same grid; k = m·2^-15 with m < 2^25, so j·m < 2^52), so a miss is
//! Gaply's and never the input's rounding. Each side meets the gate against
//! its own exact value, so the two sides may differ by the sum of their
//! gates, plus one rounding of the reference side's own arithmetic: that
//! sum is the bound used, |lhs − rhs| ≤ 1e-12 · (max(|m|, s) + max(|m'|, s'))
//! + ulp(rhs), with m, s and m', s' each side's own mean and sd.
//!
//! Also: permuting the rows changes no byte of the result; min ≤ q1 ≤
//! median ≤ q3 ≤ max; the mean lies in [min − tol, max + tol]; sd ≥ 0 and
//! exactly 0 when min equals max; counts sum to the rows; and the profile
//! and the summary agree on n and missing.

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, RngAlgorithm, RngSeed};

use gaply_core::stats_desc::{
    describe, profile, Alias, ColumnKind, ColumnRequest, ColumnResult, DescribeParams,
    DescribeResult, NumericSummary, Stat, TreatAs,
};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

const GATE: f64 = 1e-12;
const GRID: f64 = 128.0;
const SEED: u64 = 20_261_017;

fn config() -> ProptestConfig {
    ProptestConfig {
        // 5 properties × 1,024 cases: 0.9 s in a debug build (measured 17 Oct 2026).
        cases: 1024,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(SEED),
        ..ProptestConfig::default()
    }
}

/// One cell's text as a file would hold it: a number in its shortest
/// round-trip form, a label, or an empty field.
#[derive(Debug, Clone)]
enum Raw {
    Num(f64),
    Text(&'static str),
    Missing,
}

impl Raw {
    fn text(&self) -> String {
        match self {
            Raw::Num(v) => format!("{v}"),
            Raw::Text(t) => t.to_string(),
            Raw::Missing => String::new(),
        }
    }
}

fn dataset(columns: &[Vec<Raw>]) -> Dataset {
    let rows = columns.iter().map(Vec::len).max().unwrap_or(0);
    let headers: Vec<String> = (1..=columns.len()).map(|i| format!("v{i}")).collect();
    let table: Vec<Vec<String>> = (0..rows)
        .map(|r| {
            columns
                .iter()
                .map(|c| c.get(r).map(Raw::text).unwrap_or_default())
                .collect()
        })
        .collect();
    let raw = RawTable::from_rows(headers, table);
    build_dataset(
        raw,
        DataFingerprint::of_bytes(b"synthetic", "proptest", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn numeric(dataset: &Dataset, column: u32) -> NumericSummary {
    let params = DescribeParams {
        columns: vec![ColumnRequest {
            alias: Alias(ColumnAlias(column)),
            treat_as: TreatAs::Numeric,
        }],
        missing_codes: vec![],
    };
    match describe(dataset, &params)
        .expect("describes")
        .columns
        .remove(0)
    {
        ColumnResult::Numeric(s) => s,
        other => panic!("{other:?}"),
    }
}

fn value(s: Stat) -> f64 {
    match s {
        Stat::Value(v) => v,
        Stat::NotComputable(code) => panic!("not computable: {code:?}"),
    }
}

fn gate(got: f64, want: f64, scale: f64) -> bool {
    (got - want).abs() <= GATE * want.abs().max(scale)
}

/// The gate's scale for an sd: the sd itself, or 1 when it is 0.
fn nonzero(v: f64) -> f64 {
    if v == 0.0 {
        1.0
    } else {
        v
    }
}

fn ulp(x: f64) -> f64 {
    let a = x.abs();
    if a == 0.0 {
        f64::MIN_POSITIVE
    } else {
        f64::from_bits(a.to_bits() + 1) - a
    }
}

/// The exact oracle over grid values j/128: count, mean and sd from i128
/// sums, and the order statistics from the sorted values.
struct Oracle {
    n: usize,
    mean: f64,
    sd: Option<f64>,
    sorted: Vec<f64>,
}

impl Oracle {
    fn of(js: &[i64]) -> Oracle {
        let n = js.len();
        let sum: i128 = js.iter().map(|&j| i128::from(j)).sum();
        let sum_sq: i128 = js.iter().map(|&j| i128::from(j) * i128::from(j)).sum();
        let mean = sum as f64 / (GRID * n as f64);
        let sd = (n >= 2).then(|| {
            let numerator = n as i128 * sum_sq - sum * sum; // exact, ≥ 0
            ((numerator as f64) / ((n * (n - 1)) as f64 * GRID * GRID)).sqrt()
        });
        let mut sorted: Vec<f64> = js.iter().map(|&j| j as f64 / GRID).collect();
        sorted.sort_by(f64::total_cmp);
        Oracle {
            n,
            mean,
            sd,
            sorted,
        }
    }

    fn scale(&self) -> f64 {
        self.sd.filter(|s| *s != 0.0).unwrap_or(1.0)
    }

    /// R's type 7 by its definition: h = (n − 1)p, Q = (1 − g)x[j] + g·x[j+1].
    fn quantile(&self, p: f64) -> f64 {
        let h = (self.n - 1) as f64 * p;
        let j = h.floor() as usize;
        let g = h - j as f64;
        let (lo, hi) = (self.sorted[j], self.sorted[(j + 1).min(self.n - 1)]);
        if g == 0.0 || lo == hi {
            lo
        } else {
            (1.0 - g) * lo + g * hi
        }
    }
}

fn grid_value() -> impl Strategy<Value = i64> {
    -128_000_000i64..=128_000_000
}

/// Grid values with some missing cells; at least one value present.
fn grid_column() -> impl Strategy<Value = Vec<Option<i64>>> {
    prop::collection::vec(prop::option::weighted(0.8, grid_value()), 1..60)
        .prop_filter("one value", |c| c.iter().any(Option::is_some))
}

fn grid_cells(c: &[Option<i64>]) -> Vec<Raw> {
    c.iter()
        .map(|v| v.map_or(Raw::Missing, |j| Raw::Num(j as f64 / GRID)))
        .collect()
}

fn ordered(s: &NumericSummary) -> Result<(), TestCaseError> {
    let (min, q1, median, q3, max) = (
        value(s.min),
        value(s.q1),
        value(s.median),
        value(s.q3),
        value(s.max),
    );
    prop_assert!(
        min <= q1 && q1 <= median && median <= q3 && q3 <= max,
        "{min} {q1} {median} {q3} {max}"
    );
    let mean = value(s.mean);
    let sd = match s.sd {
        Stat::Value(v) => Some(v),
        _ => None,
    };
    let tol = GATE * mean.abs().max(sd.filter(|v| *v != 0.0).unwrap_or(1.0));
    prop_assert!(
        min - tol <= mean && mean <= max + tol,
        "mean {mean} outside [{min}, {max}]"
    );
    if let Some(sd) = sd {
        prop_assert!(sd >= 0.0);
        if min == max {
            prop_assert_eq!(
                sd.to_bits(),
                0f64.to_bits(),
                "a constant column's sd is exactly 0"
            );
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn the_summary_matches_the_exact_oracle(column in grid_column()) {
        let ds = dataset(&[grid_cells(&column)]);
        let s = numeric(&ds, 1);
        let js: Vec<i64> = column.iter().flatten().copied().collect();
        let o = Oracle::of(&js);
        prop_assert_eq!(s.n as usize, o.n);
        prop_assert_eq!(s.missing as usize, column.len() - o.n);
        prop_assert_eq!((s.n + s.missing) as usize, ds.rows(), "counts sum to the rows");
        prop_assert_eq!(value(s.min).to_bits(), o.sorted[0].to_bits());
        prop_assert_eq!(value(s.max).to_bits(), o.sorted[o.n - 1].to_bits());
        for (got, p) in [(s.q1, 0.25), (s.median, 0.5), (s.q3, 0.75)] {
            prop_assert_eq!(value(got).to_bits(), o.quantile(p).to_bits(), "quantile {}", p);
        }
        let mean = value(s.mean);
        prop_assert!(gate(mean, o.mean, o.scale()), "mean {:e} oracle {:e}", mean, o.mean);
        match (s.sd, o.sd) {
            (Stat::Value(sd), Some(want)) => prop_assert!(gate(sd, want, o.scale()), "sd {:e} oracle {:e}", sd, want),
            (Stat::NotComputable(_), None) => {}
            (got, want) => prop_assert!(false, "sd {:?}, oracle {:?}", got, want),
        }
        ordered(&s)?;
    }

    #[test]
    fn permuting_the_rows_changes_no_byte(
        rows in prop::collection::vec((prop::option::weighted(0.85, -1e6f64..=1e6), prop::option::weighted(0.85, grid_value())), 1..60)
            .prop_flat_map(|rows| (Just(rows.clone()), Just(rows).prop_shuffle()))
    ) {
        let (original, shuffled) = rows;
        let build = |rows: &[(Option<f64>, Option<i64>)]| {
            let a: Vec<Raw> = rows.iter().map(|(v, _)| v.map_or(Raw::Missing, Raw::Num)).collect();
            let b: Vec<Raw> = rows.iter().map(|(_, j)| j.map_or(Raw::Missing, |j| Raw::Num(j as f64 / GRID))).collect();
            dataset(&[a, b])
        };
        let params = |ds: &Dataset| DescribeParams::defaults_for(&profile(ds));
        let (d1, d2) = (build(&original), build(&shuffled));
        let r1: DescribeResult = describe(&d1, &params(&d1)).expect("describes");
        let r2: DescribeResult = describe(&d2, &params(&d2)).expect("describes");
        prop_assert_eq!(r1.canonical_bytes(), r2.canonical_bytes());
        for c in &r1.columns {
            if let ColumnResult::Numeric(s) = c {
                if s.n > 0 {
                    ordered(s)?;
                }
            }
        }
    }

    #[test]
    fn a_shift_moves_the_mean_by_the_shift(column in grid_column(), c in grid_value().prop_map(|j| j as f64 / GRID)) {
        let x = numeric(&dataset(&[grid_cells(&column)]), 1);
        let shifted: Vec<Raw> = column.iter().map(|v| v.map_or(Raw::Missing, |j| Raw::Num(j as f64 / GRID + c))).collect();
        let y = numeric(&dataset(&[shifted]), 1);
        let (mx, my) = (value(x.mean), value(y.mean));
        let sd = |s: Stat| match s { Stat::Value(v) if v != 0.0 => v, _ => 1.0 };
        let rhs = mx + c;
        let bound = GATE * (mx.abs().max(sd(x.sd)) + my.abs().max(sd(y.sd))) + ulp(rhs);
        prop_assert!((my - rhs).abs() <= bound, "mean(x + c) {:e}, mean(x) + c {:e}, bound {:e}", my, rhs, bound);
        if let (Stat::Value(sx), Stat::Value(sy)) = (x.sd, y.sd) {
            let bound = GATE * (nonzero(sx) + nonzero(sy)) + ulp(sx);
            prop_assert!((sy - sx).abs() <= bound, "sd(x + c) {:e}, sd(x) {:e}", sy, sx);
        }
    }

    #[test]
    fn a_scale_scales_the_sd_by_its_size(
        column in grid_column(),
        m in 33i64..=32_767_999,
        negative in any::<bool>(),
    ) {
        let k = if negative { -(m as f64) } else { m as f64 } / 32_768.0;
        prop_assert!((1e-3..=1e3).contains(&k.abs()));
        let x = numeric(&dataset(&[grid_cells(&column)]), 1);
        let scaled: Vec<Raw> = column.iter().map(|v| v.map_or(Raw::Missing, |j| Raw::Num(k * (j as f64 / GRID)))).collect();
        let y = numeric(&dataset(&[scaled]), 1);
        if let (Stat::Value(sx), Stat::Value(sy)) = (x.sd, y.sd) {
            let rhs = k.abs() * sx;
            let bound = GATE * (nonzero(sy) + nonzero(rhs)) + ulp(rhs);
            prop_assert!((sy - rhs).abs() <= bound, "sd(kx) {:e}, |k| sd(x) {:e}, bound {:e}", sy, rhs, bound);
            if sx == 0.0 {
                prop_assert_eq!(sy.to_bits(), 0f64.to_bits());
            }
        } else {
            prop_assert_eq!(x.sd, y.sd, "both single values");
        }
        let (mx, my) = (value(x.mean), value(y.mean));
        let rhs = k * mx;
        let sd = |s: Stat| match s { Stat::Value(v) if v != 0.0 => v, _ => 1.0 };
        let bound = GATE * (my.abs().max(sd(y.sd)) + rhs.abs().max(k.abs() * sd(x.sd))) + ulp(rhs);
        prop_assert!((my - rhs).abs() <= bound, "mean(kx) {:e}, k mean(x) {:e}", my, rhs);
    }

    #[test]
    fn the_profile_and_the_summary_agree_on_n_and_missing(
        columns in prop::collection::vec(
            prop::collection::vec(prop_oneof![
                4 => (-1e6f64..=1e6).prop_map(Raw::Num),
                1 => prop::sample::select(vec!["a", "b", "Zed", "=x", "é"]).prop_map(Raw::Text),
                1 => Just(Raw::Missing),
            ], 1..40),
            1..6,
        )
    ) {
        let ds = dataset(&columns);
        let p = profile(&ds);
        let result = describe(&ds, &DescribeParams::defaults_for(&p)).expect("describes");
        prop_assert_eq!(p.columns.len(), result.columns.len());
        for (cp, cr) in p.columns.iter().zip(&result.columns) {
            let (n, missing) = match cr {
                ColumnResult::Numeric(s) => (s.n, s.missing),
                ColumnResult::Categorical(s) => {
                    prop_assert_eq!(s.levels.iter().map(|l| l.count).sum::<u64>(), s.n, "level counts sum to n");
                    (s.n, s.missing)
                }
            };
            prop_assert_eq!((cp.n, cp.missing), (n, missing));
            prop_assert_eq!(cp.n + cp.missing, p.rows, "counts sum to the rows");
            if cp.kind == ColumnKind::Categorical {
                prop_assert_eq!(cp.levels.as_ref().map(|l| l.iter().map(|x| x.count).sum::<u64>()), Some(cp.n));
            }
        }
    }
}
