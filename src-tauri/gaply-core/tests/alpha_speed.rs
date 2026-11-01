//! **Cronbach's alpha's speed, for decision 371 (Research OS, Phase 8I).**
//! Returns at once unless `GAPLY_ALPHA_SPEED=1`, so it is never an ignored
//! test and never slows the suite. Run in a release build:
//!
//! ```text
//! GAPLY_ALPHA_SPEED=1 cargo test --release -p gaply_core --test alpha_speed -- --nocapture
//! ```
//!
//! The dataset is built directly (`build_dataset`), never through the
//! ingest's limits: the methods' largest, 128 items and 100,000 rows, every
//! row complete, two items reversed, three runs; and 30 and 64 items at
//! 100,000 rows. The estimate from the correlations' Pearson rate
//! (1.308e-8 s per pair-row, `stats_corr::seconds_per_pair_row`) is
//! 8,128 pairs × 100,000 rows ≈ 10.6 s for the inter-item correlations
//! alone, about 12 s with the items' rows. Two runs give the same bytes.
//!
//! Measured 1 Nov 2026, release, macOS aarch64: k = 30 0.346 s, k = 64
//! 1.323 s, k = 128 4.801, 4.743 and 4.739 s (5.83e-9 to 5.91e-9 s per
//! pair-row, under half the correlations' Pearson rate, whose pairwise rows
//! are gathered per pair). Under 15 s, so no budget (decision 371); the
//! figures go on the method page.

use std::time::Instant;

use gaply_core::stats_alpha::{compute, AlphaParams, ScaleEnds};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

/// `rows` rows of `k` items on a 1-to-7 scale, a hashed number per cell
/// with a shared row effect so the items correlate.
fn data(rows: usize, k: usize) -> Dataset {
    let headers: Vec<String> = (1..=k).map(|v| format!("q{v}")).collect();
    let body: Vec<Vec<String>> = (0..rows)
        .map(|r| {
            let row = (r as u64 * 2_654_435_761) % 5;
            (1..=k)
                .map(|v| {
                    let h = (r as u64 * 40_503 + v as u64 * 2_246_822_519) % 3;
                    format!("{}", 1 + row + h)
                })
                .collect()
        })
        .collect();
    build_dataset(
        RawTable::from_rows(headers, body),
        DataFingerprint::of_bytes(
            format!("speed {rows} {k}").as_bytes(),
            "alpha speed",
            TableRef::FirstTable,
        ),
    )
    .expect("dataset")
}

fn params(k: usize) -> AlphaParams {
    AlphaParams {
        items: (1..=k as u32).map(|v| Alias(ColumnAlias(v))).collect(),
        reversed: vec![Alias(ColumnAlias(2)), Alias(ColumnAlias(5))],
        scale: Some(ScaleEnds { min: 1.0, max: 7.0 }),
        missing_codes: Vec::new(),
    }
}

#[test]
fn the_largest_spec_is_measured() {
    if std::env::var("GAPLY_ALPHA_SPEED").as_deref() != Ok("1") {
        return;
    }
    assert!(!cfg!(debug_assertions), "measure in a release build");
    let rows = 100_000;
    for (k, runs) in [(30, 1), (64, 1), (128, 3)] {
        let d = data(rows, k);
        let p = params(k);
        let mut bytes = None;
        for run in 1..=runs {
            let t = Instant::now();
            let r = compute(&d, &p).expect("computed");
            let s = t.elapsed().as_secs_f64();
            let pairs = k * (k - 1) / 2;
            println!(
                "SPEED k={k} rows={rows} run={run}: {s:.3} s ({:.3e} s per pair-row over {pairs} pairs)",
                s / (pairs as f64 * rows as f64)
            );
            let b = r.canonical_bytes();
            if let Some(prev) = &bytes {
                assert_eq!(prev, &b, "the same bytes run to run");
            }
            bytes = Some(b);
        }
    }
}
