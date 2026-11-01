//! **The correlations' speed, for decision 342's budget (Research OS,
//! Phase 8H).** Returns at once unless `GAPLY_CORR_SPEED=1`, so it is never
//! an ignored test and never slows the suite. Run in a release build:
//!
//! ```text
//! GAPLY_CORR_SPEED=1 cargo test --release -p gaply_core --test corr_speed -- --nocapture
//! ```
//!
//! The dataset is built directly (`build_dataset`), never through the
//! ingest's limits. The worst case: pairwise, every variable missing a cell
//! in rows where the others have one, so each pair gathers its own rows.
//! Each coefficient alone and all three, three runs each, at 100,000 rows
//! with k = 10, 20 and 30 (stopping after the first k whose three
//! coefficients take over 15 s), at 10,000 rows with k = 30 and 60, and at
//! 1,000 rows with k = 128. Each point prints its seconds per pair-row; a
//! coefficient's rate is its largest over every point. Two runs of the same
//! spec must give the same bytes.

use std::time::Instant;

use gaply_core::stats_corr::{compute, Coefficient, CorrParams, MissingHandling};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

/// `rows` rows of `k` variables: v in row r is a hashed number with ties,
/// empty where r mod (k + 1) == v, so every variable misses rows the others
/// have.
fn data(rows: usize, k: usize) -> Dataset {
    let headers: Vec<String> = (1..=k).map(|v| format!("v{v}")).collect();
    let body: Vec<Vec<String>> = (0..rows)
        .map(|r| {
            (1..=k)
                .map(|v| {
                    if r % (k + 1) == v {
                        String::new()
                    } else {
                        let h = (r as u64 * 2_654_435_761 + v as u64 * 40_503) % 100_003;
                        format!("{}", (h as f64) / 100.0 + (r % 97) as f64 * v as f64 * 0.01)
                    }
                })
                .collect()
        })
        .collect();
    build_dataset(
        RawTable::from_rows(headers, body),
        DataFingerprint::of_bytes(
            format!("speed {rows} {k}").as_bytes(),
            "corr speed",
            TableRef::FirstTable,
        ),
    )
    .expect("dataset")
}

fn params(k: usize, coefficients: &[Coefficient]) -> CorrParams {
    CorrParams {
        variables: (1..=k as u32).map(|v| Alias(ColumnAlias(v))).collect(),
        coefficients: coefficients.to_vec(),
        missing: MissingHandling::Pairwise,
        missing_codes: Vec::new(),
    }
}

#[test]
fn the_three_rates_from_the_worst_case() {
    if std::env::var("GAPLY_CORR_SPEED").as_deref() != Ok("1") {
        return;
    }
    assert!(!cfg!(debug_assertions), "measure in a release build");
    let sets: [(&str, Vec<Coefficient>); 4] = [
        ("pearson", vec![Coefficient::Pearson]),
        ("spearman", vec![Coefficient::Spearman]),
        ("kendall", vec![Coefficient::Kendall]),
        (
            "all",
            vec![
                Coefficient::Pearson,
                Coefficient::Spearman,
                Coefficient::Kendall,
            ],
        ),
    ];
    let mut worst = [0.0f64; 3];
    let points: [(usize, &[usize]); 3] = [
        (100_000, &[10, 20, 30]),
        (10_000, &[30, 60]),
        (1_000, &[128]),
    ];
    for (rows, ks) in points {
        for &k in ks {
            let d = data(rows, k);
            let pairs = (k * (k - 1) / 2) as f64;
            let mut all_slowest = 0.0f64;
            for (name, coefs) in &sets {
                let p = params(k, coefs);
                let mut times = Vec::new();
                let mut first: Option<Vec<u8>> = None;
                for _ in 0..3 {
                    let t = Instant::now();
                    let r = compute(&d, &p).expect("computes");
                    times.push(t.elapsed().as_secs_f64());
                    let bytes = r.canonical_bytes();
                    match &first {
                        None => first = Some(bytes),
                        Some(b) => assert!(*b == bytes, "two runs differ at {rows} x {k} {name}"),
                    }
                }
                let slowest = times.iter().cloned().fold(0.0, f64::max);
                let rate = slowest / (pairs * rows as f64);
                println!(
                    "POINT rows={rows} k={k} pairs={pairs} {name}: runs {:.3} {:.3} {:.3} s, slowest {:.3} s, {:.3e} s per pair-row",
                    times[0], times[1], times[2], slowest, rate
                );
                match *name {
                    "pearson" => worst[0] = worst[0].max(rate),
                    "spearman" => worst[1] = worst[1].max(rate),
                    "kendall" => worst[2] = worst[2].max(rate),
                    _ => all_slowest = slowest,
                }
            }
            if rows == 100_000 && all_slowest > 15.0 {
                println!("STOP at rows={rows} k={k}: all three took {all_slowest:.3} s, over 15 s");
                break;
            }
        }
    }
    println!(
        "RATES pearson {:.3e} spearman {:.3e} kendall {:.3e} s per pair-row",
        worst[0], worst[1], worst[2]
    );
}
