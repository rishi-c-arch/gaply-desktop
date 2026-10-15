//! **The sweep that measures the data-precision range's validity limit L and
//! widening F** (Phase 2 evaluation fixes, part A, section 2). `#[ignore]`:
//! it searches thousands of boxes and is run on purpose, in release:
//!
//! ```text
//! SWEEP_OUT=/path/sweep.tsv cargo test --release -p gaply_core \
//!     --test l1_precision_sweep -- --ignored --nocapture
//! ```
//!
//! For every method, at least 200 seeded datasets with the ratio r (δ over
//! the smallest standard deviation among the bound groups or columns) spread
//! log-uniformly from 0.005 to 1, plus the joint evaluation's witness and its
//! five section-3 datasets (`tests/fixtures/stats_l1/precision_sweep/`).
//! For each, the TRUE extremes of the statistic over the box `x ± δ` are
//! found by an adversarial search, not by uniform draws, which rarely reach
//! the corners: from both sign corners, the stored cells and four seeded
//! starts, coordinate ascent (and descent) over five positions per cell,
//! repeated until no move improves.
//!
//! Each row reports the widening the hull needed to contain both extremes.
//! L and F are chosen from the rows: L is the largest r below which the
//! needed widening stays at or under F_MAX (2.0), then halved for margin; F
//! is the largest widening needed at or below the halved L, rounded up to a
//! tenth. The second test asserts the shipped constants contain every
//! extreme found at or below L.

use gaply_core::stats_l1::precision::{
    self, half_unit, independent_t_parts, ols_parts, one_way_f_parts, paired_t_parts,
    pearson_parts, RangeParts, StatRange, Stored,
};
use gaply_core::stats_verify;

/* ---- a seeded generator -------------------------------------------------- */

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: usize, hi: usize) -> usize {
        lo + (self.next() % (hi - lo + 1) as u64) as usize
    }
    fn normal(&mut self) -> f64 {
        let (u, v) = (self.unit().max(1e-300), self.unit());
        (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
    }
}

/* ---- one statistic over one box ---------------------------------------- */

type Stat = Box<dyn Fn(&[f64]) -> Option<f64>>;

struct Item {
    method: &'static str,
    case: String,
    cells: Vec<f64>,
    deltas: Vec<f64>,
    parts: RangeParts,
    stat: Stat,
}

/// The extreme of `stat` over the box, in direction `sign` (+1 max, -1 min).
fn extreme(item: &Item, sign: f64, rng: &mut Rng) -> f64 {
    let n = item.cells.len();
    let eval = |v: &[f64]| (item.stat)(v).map(|s| sign * s).filter(|s| s.is_finite());
    let mut starts: Vec<Vec<f64>> = vec![item.cells.clone()];
    // The two sign corners, by the statistic's own finite differences.
    let base = (item.stat)(&item.cells).unwrap_or(f64::NAN);
    let dir: Vec<f64> = (0..n)
        .map(|i| {
            let mut v = item.cells.clone();
            v[i] += item.deltas[i] * 1e-3;
            let s = (item.stat)(&v).unwrap_or(base);
            if s > base {
                1.0
            } else if s < base {
                -1.0
            } else {
                0.0
            }
        })
        .collect();
    for sg in [1.0, -1.0] {
        starts.push(
            item.cells
                .iter()
                .zip(&item.deltas)
                .zip(&dir)
                .map(|((x, d), g)| x + sg * d * g)
                .collect(),
        );
    }
    for _ in 0..4 {
        starts.push(
            item.cells
                .iter()
                .zip(&item.deltas)
                .map(|(x, d)| x + d * (2.0 * rng.unit() - 1.0))
                .collect(),
        );
    }
    let mut best = f64::NEG_INFINITY;
    for mut x in starts {
        let mut cur = match eval(&x) {
            Some(s) => s,
            None => continue,
        };
        for _pass in 0..30 {
            let mut improved = false;
            for i in 0..n {
                let keep = x[i];
                let mut best_i = (cur, keep);
                for o in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                    x[i] = item.cells[i] + o * item.deltas[i];
                    if let Some(s) = eval(&x) {
                        if s > best_i.0 * (1.0 + 1e-14 * best_i.0.signum()) + 1e-300 {
                            best_i = (s, x[i]);
                        }
                    }
                }
                x[i] = best_i.1;
                if best_i.0 > cur {
                    cur = best_i.0;
                    improved = true;
                }
            }
            if !improved {
                break;
            }
        }
        best = best.max(cur);
    }
    sign * best
}

/// The widening the hull needs to contain `lo` and `hi`.
fn needed(parts: &RangeParts, lo: f64, hi: f64) -> f64 {
    let (hl, hh) = parts.hull();
    let side = |extent: f64, hull: f64| {
        if extent <= 1e-12 * parts.value.abs().max(1.0) {
            0.0
        } else if hull <= 0.0 {
            f64::INFINITY
        } else {
            extent / hull
        }
    };
    side(parts.value - lo, parts.value - hl).max(side(hi - parts.value, hh - parts.value))
}

/* ---- the datasets ------------------------------------------------------- */

fn round(x: f64, d: u32) -> f64 {
    let s = 10f64.powi(d as i32);
    (x * s).round() / s
}

/// Scale groups of raw values so that the smallest group SD is δ / r, shift,
/// and round to `d` decimals.
fn stored(groups: &[Vec<f64>], r: f64, d: u32, shift: f64) -> Vec<Vec<f64>> {
    let sd = |v: &[f64]| {
        let m = v.iter().sum::<f64>() / v.len() as f64;
        (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() as f64 - 1.0)).sqrt()
    };
    let min_sd = groups.iter().map(|g| sd(g)).fold(f64::INFINITY, f64::min);
    let scale = half_unit(d) / r / min_sd;
    groups
        .iter()
        .map(|g| g.iter().map(|x| round(shift + x * scale, d)).collect())
        .collect()
}

fn t_items(case: String, a: Vec<f64>, c: Vec<f64>, d: u32) -> Vec<Item> {
    let delta = half_unit(d);
    let p = independent_t_parts(&a, &c, delta);
    let cells: Vec<f64> = a.iter().chain(&c).copied().collect();
    let n1 = a.len();
    let mk = |method, parts, which: u8| Item {
        method,
        case: case.clone(),
        cells: cells.clone(),
        deltas: vec![delta; cells.len()],
        parts,
        stat: Box::new(move |v: &[f64]| match which {
            0 => stats_verify::t_test_student(&v[..n1], &v[n1..])
                .ok()
                .map(|r| r.t),
            1 => stats_verify::t_test_welch(&v[..n1], &v[n1..])
                .ok()
                .map(|r| r.t),
            _ => stats_verify::t_test_welch(&v[..n1], &v[n1..])
                .ok()
                .map(|r| r.df),
        }),
    };
    vec![
        mk("student_t", p.student, 0),
        mk("welch_t", p.welch, 1),
        mk("welch_df", p.welch_df, 2),
    ]
}

fn paired_item(case: String, x: Vec<f64>, y: Vec<f64>, d: u32) -> Item {
    let delta = half_unit(d);
    let n = x.len();
    let parts = paired_t_parts(Stored { values: &x, delta }, Stored { values: &y, delta });
    let cells: Vec<f64> = x.iter().chain(&y).copied().collect();
    Item {
        method: "paired_t",
        case,
        deltas: vec![delta; cells.len()],
        cells,
        parts,
        stat: Box::new(move |v: &[f64]| {
            stats_verify::t_test_paired(&v[..n], &v[n..])
                .ok()
                .map(|r| r.t)
        }),
    }
}

fn anova_item(case: String, groups: Vec<Vec<f64>>, d: u32) -> Item {
    let delta = half_unit(d);
    let refs: Vec<&[f64]> = groups.iter().map(Vec::as_slice).collect();
    let parts = one_way_f_parts(&refs, delta);
    let sizes: Vec<usize> = groups.iter().map(Vec::len).collect();
    let cells: Vec<f64> = groups.iter().flatten().copied().collect();
    Item {
        method: "one_way_f",
        case,
        deltas: vec![delta; cells.len()],
        cells,
        parts,
        stat: Box::new(move |v: &[f64]| {
            let mut at = 0;
            let gs: Vec<&[f64]> = sizes
                .iter()
                .map(|&k| {
                    let g = &v[at..at + k];
                    at += k;
                    g
                })
                .collect();
            stats_verify::one_way_anova(&gs).ok().map(|r| r.f)
        }),
    }
}

fn pearson_item(case: String, x: Vec<f64>, y: Vec<f64>, d: u32) -> Item {
    let delta = half_unit(d);
    let n = x.len();
    let parts = pearson_parts(Stored { values: &x, delta }, Stored { values: &y, delta });
    let cells: Vec<f64> = x.iter().chain(&y).copied().collect();
    Item {
        method: "pearson_r",
        case,
        deltas: vec![delta; cells.len()],
        cells,
        parts,
        stat: Box::new(move |v: &[f64]| {
            stats_verify::pearson(&v[..n], &v[n..])
                .ok()
                .map(|r| r.coefficient)
        }),
    }
}

fn ols_items(case: String, y: Vec<f64>, xs: Vec<Vec<f64>>, d: u32) -> Vec<Item> {
    let delta = half_unit(d);
    let n = y.len();
    let k = xs.len();
    let Ok(fit) = stats_verify::ols_qr(&y, &xs) else {
        return Vec::new();
    };
    let beta: Vec<f64> = fit.coefficients.iter().map(|c| c.estimate).collect();
    let stored_xs: Vec<Stored<'_>> = xs.iter().map(|x| Stored { values: x, delta }).collect();
    let Some((f, ts)) = ols_parts(Stored { values: &y, delta }, &stored_xs, &beta) else {
        return Vec::new();
    };
    let cells: Vec<f64> = y
        .iter()
        .copied()
        .chain(xs.iter().flatten().copied())
        .collect();
    let split = move |v: &[f64]| -> (Vec<f64>, Vec<Vec<f64>>) {
        (
            v[..n].to_vec(),
            (0..k)
                .map(|l| v[n * (l + 1)..n * (l + 2)].to_vec())
                .collect(),
        )
    };
    let mut out = vec![Item {
        method: "ols_f",
        case: case.clone(),
        deltas: vec![delta; cells.len()],
        cells: cells.clone(),
        parts: f,
        stat: Box::new(move |v: &[f64]| {
            let (yy, xx) = split(v);
            stats_verify::ols_qr(&yy, &xx).ok().map(|r| r.f_statistic)
        }),
    }];
    out.push(Item {
        method: "ols_slope_t",
        case,
        deltas: vec![delta; cells.len()],
        cells,
        parts: ts[1],
        stat: Box::new(move |v: &[f64]| {
            let (yy, xx) = split(v);
            stats_verify::ols_qr(&yy, &xx)
                .ok()
                .map(|r| r.coefficients[1].t)
        }),
    });
    out
}

/// Seeded datasets for every method, `per_method` each.
fn generated(per_method: usize) -> Vec<Item> {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut items = Vec::new();
    for i in 0..per_method {
        let r = 0.005 * 200f64.powf(i as f64 / (per_method - 1) as f64);
        let d = if rng.unit() < 0.5 { 1 } else { 2 };
        let shift = 10.0 * rng.unit();
        // Independent t: two groups, unequal sizes and spreads.
        let (n1, n2) = (rng.range(2, 15), rng.range(2, 15));
        let eff = 1.5 * rng.unit();
        let s2 = 0.5 + 2.0 * rng.unit();
        let a: Vec<f64> = (0..n1).map(|_| rng.normal()).collect();
        let c: Vec<f64> = (0..n2).map(|_| eff + s2 * rng.normal()).collect();
        let g = stored(&[a, c], r, d, shift);
        items.extend(t_items(format!("gen{i}"), g[0].clone(), g[1].clone(), d));
        // Paired: correlated columns; r set on the differences' spread.
        let n = rng.range(3, 20);
        let x: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let shiftd = 1.0 * rng.unit();
        let noise = 0.2 + rng.unit();
        let y: Vec<f64> = x
            .iter()
            .map(|v| v + shiftd + noise * rng.normal())
            .collect();
        let sd_d = {
            let dd: Vec<f64> = x.iter().zip(&y).map(|(a, b)| a - b).collect();
            let m = dd.iter().sum::<f64>() / n as f64;
            (dd.iter().map(|z| (z - m).powi(2)).sum::<f64>() / (n as f64 - 1.0)).sqrt()
        };
        let k = half_unit(d) / r / sd_d;
        let xs: Vec<f64> = x.iter().map(|v| round(shift + v * k, d)).collect();
        let ys: Vec<f64> = y.iter().map(|v| round(shift + v * k, d)).collect();
        items.push(paired_item(format!("gen{i}"), xs, ys, d));
        // One-way: 2 to 5 groups.
        let groups: Vec<Vec<f64>> = (0..rng.range(2, 5))
            .map(|_| {
                let m = 1.5 * rng.unit();
                (0..rng.range(2, 10)).map(|_| m + rng.normal()).collect()
            })
            .collect();
        items.push(anova_item(
            format!("gen{i}"),
            stored(&groups, r, d, shift),
            d,
        ));
        // Pearson: any correlation, including near +-1.
        let n = rng.range(4, 25);
        let rho = (2.0 * rng.unit() - 1.0) * 0.995;
        let x: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let y: Vec<f64> = x
            .iter()
            .map(|v| rho * v + (1.0 - rho * rho).sqrt() * rng.normal())
            .collect();
        let g = stored(&[x, y], r, d, shift);
        items.push(pearson_item(
            format!("gen{i}"),
            g[0].clone(),
            g[1].clone(),
            d,
        ));
        // OLS: 1 to 3 predictors, sometimes near-collinear.
        let n = rng.range(6, 30);
        let k = rng.range(1, 3);
        let mut xs: Vec<Vec<f64>> = (0..k)
            .map(|_| (0..n).map(|_| rng.normal()).collect())
            .collect();
        if k > 1 && rng.unit() < 0.4 {
            let x0 = xs[0].clone();
            xs[1] = x0.iter().map(|v| v + 0.05 * rng.normal()).collect();
        }
        let betas: Vec<f64> = (0..k).map(|_| 2.0 * rng.unit() - 1.0).collect();
        let y: Vec<f64> = (0..n)
            .map(|i| xs.iter().zip(&betas).map(|(x, b)| x[i] * b).sum::<f64>() + rng.normal())
            .collect();
        let mut all = vec![y];
        all.extend(xs);
        let g = stored(&all, r, d, shift);
        items.extend(ols_items(
            format!("gen{i}"),
            g[0].clone(),
            g[1..].to_vec(),
            d,
        ));
    }
    items
}

/// The evaluation's datasets, read from the fixture folder.
fn fixtures() -> Vec<Item> {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/stats_l1/precision_sweep/"
    );
    let read = |name: &str| -> (Vec<String>, Vec<Vec<String>>) {
        let text = std::fs::read_to_string(format!("{dir}{name}.csv")).expect("fixture");
        assert!(
            text.lines()
                .any(|l| l.starts_with('#') && l.contains("/usr/local/bin/Rscript")),
            "{name} names its generator"
        );
        let mut lines = text.lines().filter(|l| !l.starts_with('#'));
        let header = lines.next().unwrap().split(',').map(String::from).collect();
        // Rows with a missing cell are dropped, listwise, as Level 1 drops them.
        let rows: Vec<Vec<String>> = lines
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split(',').map(String::from).collect::<Vec<String>>())
            .filter(|r| r.iter().all(|c| !c.trim().is_empty()))
            .collect();
        (header, rows)
    };
    let num = |s: &str| s.parse::<f64>().unwrap();
    let decimals = |v: &[f64]| v.iter().map(|x| precision::decimals(*x)).max().unwrap();
    let grouped = |rows: &[Vec<String>]| -> Vec<Vec<f64>> {
        let mut labels: Vec<String> = Vec::new();
        for r in rows {
            if !labels.contains(&r[1]) {
                labels.push(r[1].clone());
            }
        }
        labels
            .iter()
            .map(|l| {
                rows.iter()
                    .filter(|r| &r[1] == l)
                    .map(|r| num(&r[0]))
                    .collect()
            })
            .collect()
    };
    let mut items = Vec::new();
    let (_, rows) = read("c02_t_welch_1dec");
    let g = grouped(&rows);
    let d = decimals(&g.concat());
    items.extend(t_items(
        "c02_t_welch_1dec".into(),
        g[0].clone(),
        g[1].clone(),
        d,
    ));
    let (_, rows) = read("c06_paired_2dec");
    let (x, y): (Vec<f64>, Vec<f64>) = rows.iter().map(|r| (num(&r[0]), num(&r[1]))).unzip();
    let d = decimals(&[x.clone(), y.clone()].concat());
    items.push(paired_item("c06_paired_2dec".into(), x, y, d));
    for name in ["c08_anova3_1dec", "s1_anova_tiny_ssw"] {
        let (_, rows) = read(name);
        let g = grouped(&rows);
        let d = decimals(&g.concat());
        items.push(anova_item(name.into(), g, d));
    }
    for name in ["c11_pearson_neg_2dec", "s2_pearson_near1"] {
        let (_, rows) = read(name);
        let (x, y): (Vec<f64>, Vec<f64>) = rows.iter().map(|r| (num(&r[0]), num(&r[1]))).unzip();
        let d = decimals(&[x.clone(), y.clone()].concat());
        items.push(pearson_item(name.into(), x, y, d));
    }
    let (_, rows) = read("c18_ols1_2dec");
    let (y, x): (Vec<f64>, Vec<f64>) = rows.iter().map(|r| (num(&r[0]), num(&r[1]))).unzip();
    let d = decimals(&[y.clone(), x.clone()].concat());
    items.extend(ols_items("c18_ols1_2dec".into(), y, vec![x], d));
    items
}

struct Row {
    method: &'static str,
    case: String,
    ratio: f64,
    needed: f64,
    first_order_covers: bool,
    lo: f64,
    hi: f64,
    value: f64,
}

fn sweep() -> Vec<Row> {
    let mut items = generated(210);
    items.extend(fixtures());
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    items
        .iter()
        .filter(|it| it.parts.value.is_finite())
        .map(|it| {
            let (lo, hi) = (extreme(it, -1.0, &mut rng), extreme(it, 1.0, &mut rng));
            let b = it.parts.first_order;
            let first_order_covers = lo >= it.parts.value - b && hi <= it.parts.value + b;
            Row {
                method: it.method,
                case: it.case.clone(),
                ratio: it.parts.ratio,
                needed: needed(&it.parts, lo, hi),
                first_order_covers,
                lo,
                hi,
                value: it.parts.value,
            }
        })
        .collect()
}

#[test]
#[ignore = "the precision sweep: minutes in release; run on purpose"]
fn measure_the_validity_limit_and_the_widening() {
    let rows = sweep();
    let mut out =
        String::from("method\tcase\tratio\tneeded\tfirst_order_covers\tvalue\ttrue_lo\ttrue_hi\n");
    for r in &rows {
        out.push_str(&format!(
            "{}\t{}\t{:.6}\t{:.6}\t{}\t{:.10}\t{:.10}\t{:.10}\n",
            r.method, r.case, r.ratio, r.needed, r.first_order_covers, r.value, r.lo, r.hi
        ));
    }
    if let Ok(p) = std::env::var("SWEEP_OUT") {
        std::fs::write(p, &out).expect("write sweep");
    }
    let methods: std::collections::BTreeSet<&str> = rows.iter().map(|r| r.method).collect();
    for m in &methods {
        let n = rows.iter().filter(|r| r.method == *m).count();
        println!("{m}: {n} datasets");
        assert!(n >= 200, "{m}: only {n} datasets");
    }
    // The choice, by the rule in the module header.
    const F_MAX: f64 = 2.0;
    let mut sorted: Vec<&Row> = rows.iter().collect();
    sorted.sort_by(|a, b| a.ratio.total_cmp(&b.ratio));
    let mut l = 0.0;
    let mut worst = 0.0f64;
    for r in &sorted {
        worst = worst.max(r.needed);
        if worst > F_MAX {
            break;
        }
        l = r.ratio;
    }
    let l_half = l / 2.0;
    let f = rows
        .iter()
        .filter(|r| r.ratio <= l_half)
        .map(|r| r.needed)
        .fold(0.0f64, f64::max);
    let worst_row = rows
        .iter()
        .filter(|r| r.ratio <= l_half)
        .max_by(|a, b| a.needed.total_cmp(&b.needed))
        .unwrap();
    println!("L before halving = {l:.6}; L = {l_half:.6}; widening needed at or below L = {f:.6} (worst: {} {} r = {:.6})", worst_row.method, worst_row.case, worst_row.ratio);
    println!(
        "first-order alone fails on {} of {} rows",
        rows.iter().filter(|r| !r.first_order_covers).count(),
        rows.len()
    );
}

/// The shipped constants contain every extreme the sweep finds at or below
/// L; above L the range is unbounded.
#[test]
#[ignore = "the precision sweep: minutes in release; run on purpose"]
fn the_shipped_limit_and_widening_contain_every_extreme_found() {
    let rows = sweep();
    let mut misses = Vec::new();
    for r in &rows {
        if r.ratio > precision::VALIDITY_LIMIT {
            continue;
        }
        if r.needed > precision::WIDENING {
            misses.push(format!(
                "{} {} r = {:.6}: needs {:.4}",
                r.method, r.case, r.ratio, r.needed
            ));
        }
    }
    let covered = rows
        .iter()
        .filter(|r| r.ratio <= precision::VALIDITY_LIMIT)
        .count();
    println!(
        "{covered} of {} rows at or below L = {}; F = {}",
        rows.len(),
        precision::VALIDITY_LIMIT,
        precision::WIDENING
    );
    assert!(covered > 0, "no row at or below L: the check is inert");
    assert!(
        misses.is_empty(),
        "{} rows not contained:\n{}",
        misses.len(),
        misses.join("\n")
    );
    let _ = StatRange::Exact;
}
