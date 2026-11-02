//! **Scale scores' metamorphic tests and the columns' pin (Research OS, Phase
//! 8M, decision 409, ADR-011 Part 13, ADR-008 §5).**
//!
//! Tests with no oracle, by proptest on a binary grid, so a transformed input
//! is exact and a miss is Gaply's, never the input's. Each relation states its
//! tolerance: exact where IEEE arithmetic makes it exact on this grid.
//!
//! * Reversing twice gives the value back.
//! * A reversed item and the same item unreversed add to k(min + max) over
//!   the k items of a complete row.
//! * Alpha's scale mean if deleted equals the summary's mean of a sum score
//!   over the other items, on a complete table.
//! * The prepared CSV read back gives the same scores, read by a test-local RFC
//!   4180 reader, then `RawTable::from_rows` and `build_dataset`, and remade
//!   with the same recipe.
//!
//! The pin: a SHA-256 of the prepared columns' bits for a fixed case, asserted
//! exactly on every platform, since the build uses only addition, subtraction
//! and division (ADR-011 Part 7).

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, RngAlgorithm, RngSeed};

use gaply_core::contracts::CapabilityId;
use gaply_core::stats_alpha::{self, AlphaParams, ScaleEnds, Stat as AlphaStat};
use gaply_core::stats_desc::{
    describe, Alias, ColumnRequest, ColumnResult, DescribeParams, Stat, TreatAs,
};
use gaply_core::stats_l1::dataset::{
    build_dataset, sha256_hex, Cell, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_prep::{
    build, export::prepared_csv, MissingRule, Prepared, Recipe, RecipeStep, ScaleScoreStep,
    Statistic, CAPABILITY_ID, CAPABILITY_VERSION, ENGINE_VERSION, RECIPE_SCHEMA_VERSION,
};

const CAP: u64 = 5_000_000;
const SEED: u64 = 20_261_102;
/// The scale's ends: dyadic, so every reversal on the grid is exact.
const MIN: f64 = -2.0;
const MAX: f64 = 6.0;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 256,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(SEED),
        ..ProptestConfig::default()
    }
}

fn a(n: u32) -> Alias {
    Alias(ColumnAlias(n))
}

fn parsed(headers: Vec<String>, rows: Vec<Vec<String>>, bytes: &[u8]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(bytes, "prep properties", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// A table of the grid's values: a row, then its columns.
fn table(rows: &[Vec<f64>]) -> Dataset {
    let k = rows[0].len();
    parsed(
        (1..=k).map(|j| format!("i{j}")).collect(),
        rows.iter()
            .map(|r| r.iter().map(|v| format!("{v:?}")).collect())
            .collect(),
        b"grid",
    )
}

fn recipe(columns: u32, step: ScaleScoreStep) -> Recipe {
    Recipe {
        schema_version: RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(CAPABILITY_ID).unwrap(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        source_columns: columns,
        steps: vec![RecipeStep::ScaleScore(step)],
    }
}

fn step(k: u32, items: Vec<Alias>, reversed: Vec<Alias>) -> ScaleScoreStep {
    let scale = (!reversed.is_empty()).then_some(ScaleEnds { min: MIN, max: MAX });
    ScaleScoreStep {
        alias: a(k + 1),
        name: "the score".to_string(),
        items,
        reversed,
        scale,
        statistic: Some(Statistic::Sum),
        rule: Some(MissingRule::EveryItem),
        missing_codes: Vec::new(),
    }
}

fn scores(p: &Prepared, alias: u32) -> Vec<Option<f64>> {
    p.dataset
        .column_cells(ColumnAlias(alias))
        .expect("the score")
        .iter()
        .map(Cell::number)
        .collect()
}

/// Whole quarters from MIN to MAX: exact in binary, in every sum here.
fn grid_value() -> impl Strategy<Value = f64> {
    (-8i32..=24).prop_map(|q| f64::from(q) / 4.0)
}

/// 3 to 12 rows of 2 to 6 grid values.
fn grid_rows() -> impl Strategy<Value = Vec<Vec<f64>>> {
    (2usize..=6, 3usize..=12)
        .prop_flat_map(|(k, n)| prop::collection::vec(prop::collection::vec(grid_value(), k), n))
}

/* ---- a test-local RFC 4180 reader, on tests/desc_export.rs's pattern ---- */

/// RFC 4180: CRLF between records, `"` around a field, `""` inside one; then,
/// for a quoted field, exactly one leading apostrophe is removed.
fn read_csv(bytes: &[u8]) -> Vec<Vec<String>> {
    let s = std::str::from_utf8(bytes).expect("utf-8");
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut chars = s.chars().peekable();
    loop {
        let mut field = String::new();
        let mut quoted = false;
        if chars.peek() == Some(&'"') {
            quoted = true;
            chars.next();
            loop {
                match chars.next().expect("a closing quote") {
                    '"' if chars.peek() == Some(&'"') => {
                        chars.next();
                        field.push('"');
                    }
                    '"' => break,
                    c => field.push(c),
                }
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c == ',' || c == '\r' {
                    break;
                }
                field.push(c);
                chars.next();
            }
        }
        if quoted && field.starts_with('\'') {
            field.remove(0);
        }
        record.push(field);
        match chars.next() {
            Some(',') => continue,
            Some('\r') => {
                assert_eq!(chars.next(), Some('\n'), "a record ends with CRLF");
                records.push(std::mem::take(&mut record));
                if chars.peek().is_none() {
                    return records;
                }
            }
            None => panic!("the last record has no CRLF"),
            Some(c) => panic!("unexpected {c:?} after a field"),
        }
    }
}

proptest! {
    #![proptest_config(config())]

    /// Reversing a column, then reversing what that made, gives the value
    /// back: the second score over the first's reversed cells.
    #[test]
    fn reversing_twice_gives_the_value_back(rows in grid_rows()) {
        let k = rows[0].len() as u32;
        let d = table(&rows);
        let items: Vec<Alias> = (1..=k).map(a).collect();
        let once = build(&d, &recipe(k, step(k, items.clone(), items.clone())), CAP).unwrap();
        // The reversed table, as a file would hold it.
        let flipped: Vec<Vec<f64>> = rows
            .iter()
            .map(|r| r.iter().map(|v| MIN + MAX - v).collect())
            .collect();
        let twice = build(
            &table(&flipped),
            &recipe(k, step(k, items.clone(), items.clone())),
            CAP,
        )
        .unwrap();
        let plain = build(&d, &recipe(k, step(k, items, Vec::new())), CAP).unwrap();
        prop_assert_eq!(scores(&twice, k + 1), scores(&plain, k + 1));
        prop_assert_eq!(scores(&once, k + 1).len(), rows.len());
    }

    /// On a complete row, the score with every item reversed plus the score
    /// with none adds to k(min + max), exactly on this grid.
    #[test]
    fn reversed_plus_unreversed_adds_to_k_times_min_plus_max(rows in grid_rows()) {
        let k = rows[0].len() as u32;
        let d = table(&rows);
        let items: Vec<Alias> = (1..=k).map(a).collect();
        let plain = build(&d, &recipe(k, step(k, items.clone(), Vec::new())), CAP).unwrap();
        let reversed = build(&d, &recipe(k, step(k, items.clone(), items)), CAP).unwrap();
        let want = f64::from(k) * (MIN + MAX);
        for (p, r) in scores(&plain, k + 1).into_iter().zip(scores(&reversed, k + 1)) {
            prop_assert_eq!(p.expect("complete") + r.expect("complete"), want);
        }
    }

    /// Alpha's scale mean if an item is deleted, against the descriptive
    /// summary's mean of a sum score over the other items, on a complete
    /// table. Both come from the one mean; on this grid the row totals are
    /// exact, so both are means of the same doubles, and the only difference
    /// allowed is the order they are fed in: alpha's are the values ascending.
    #[test]
    fn alphas_scale_mean_if_deleted_is_the_summarys_mean_of_a_sum_of_the_others(rows in grid_rows()) {
        let k = rows[0].len() as u32;
        prop_assume!(k >= 3);
        let d = table(&rows);
        let items: Vec<Alias> = (1..=k).map(a).collect();
        let Ok(outcome) = stats_alpha::compute(
            &d,
            &AlphaParams {
                items: items.clone(),
                reversed: Vec::new(),
                scale: None,
                missing_codes: Vec::new(),
            },
        ) else {
            return Ok(());
        };
        for left_out in 1..=k {
            let others: Vec<Alias> = (1..=k).filter(|j| *j != left_out).map(a).collect();
            let p = build(&d, &recipe(k, step(k, others, Vec::new())), CAP).unwrap();
            let score = a(k + 1);
            let summary = describe(
                &p.dataset,
                &DescribeParams {
                    columns: vec![ColumnRequest { alias: score, treat_as: TreatAs::Numeric }],
                    missing_codes: Vec::new(),
                },
            )
            .unwrap();
            let ColumnResult::Numeric(s) = &summary.columns[0] else {
                panic!("a score is a number")
            };
            let Stat::Value(mean) = s.mean else {
                panic!("the mean of a complete score")
            };
            let AlphaStat::Value(alpha_mean) = outcome.items[left_out as usize - 1].scale_mean_if_deleted else {
                panic!("the scale mean if deleted")
            };
            // Means of the same values on a 1/4 grid over at most 12 rows: the
            // two orders may differ by rounding of the mean, scaled by the
            // largest total's size: k * max(|MIN|, |MAX|).
            let scale = f64::from(k) * 6.0 * f64::EPSILON;
            prop_assert!(
                (mean - alpha_mean).abs() <= scale,
                "item {}: {:e} against {:e}", left_out, mean, alpha_mean
            );
        }
    }

    /// The CSV read back through a real reader and Level 1 gives the same
    /// numbers, and the same recipe on what was read gives the same scores.
    #[test]
    fn the_csv_read_back_gives_the_same_scores_and_the_recipe_remakes_them(
        rows in grid_rows(),
        statistic in prop_oneof![Just(Statistic::Sum), Just(Statistic::Mean)],
        at_least in any::<bool>(),
        holes in prop::collection::vec((0usize..12, 0usize..6), 0..6),
    ) {
        let k = rows[0].len() as u32;
        let cells: Vec<Vec<String>> = rows
            .iter()
            .enumerate()
            .map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .map(|(c, v)| {
                        if holes.contains(&(r, c)) {
                            String::new()
                        } else {
                            format!("{v:?}")
                        }
                    })
                    .collect()
            })
            .collect();
        let d = parsed((1..=k).map(|j| format!("i{j}")).collect(), cells, b"holes");
        let items: Vec<Alias> = (1..=k).map(a).collect();
        let mut s = step(k, items.clone(), vec![items[0]]);
        s.statistic = Some(statistic);
        s.name = "the, \"score\"".to_string();
        if at_least {
            s.rule = Some(MissingRule::AtLeast(1));
        }
        let r = recipe(k, s);
        let Ok(p) = build(&d, &r, CAP) else {
            // Holes can leave no complete row: a refusal, nothing to read back.
            return Ok(());
        };
        let records = read_csv(&prepared_csv(&p));
        prop_assert_eq!(records.len(), rows.len() + 1);
        let headers = records[0].clone();
        prop_assert_eq!(&headers[k as usize], "the, \"score\"");
        let back = parsed(headers, records[1..].to_vec(), b"read back");

        // The same numbers in every column, the scores included.
        for j in 1..=k + 1 {
            let have: Vec<Option<f64>> = p.dataset.column_cells(ColumnAlias(j)).unwrap().iter().map(Cell::number).collect();
            let read: Vec<Option<f64>> = back.column_cells(ColumnAlias(j)).unwrap().iter().map(Cell::number).collect();
            prop_assert_eq!(have, read, "column {}", j);
        }

        // The file's columns of what was read, remade with the same recipe.
        let file_only = parsed(
            records[0][..k as usize].to_vec(),
            records[1..].iter().map(|row| row[..k as usize].to_vec()).collect(),
            b"file columns",
        );
        let again = build(&file_only, &r, CAP).unwrap();
        prop_assert_eq!(scores(&again, k + 1), scores(&p, k + 1));
    }
}

/* ---- the columns' pin ---------------------------------------------------- */

/// A fixed case in every shape the build has: reversal on fractional ends, a
/// ticked code, tokens, fractions that do not fit binary, a sum and a mean
/// under each rule.
fn pin_case() -> (Dataset, Recipe) {
    let rows: Vec<Vec<String>> = (0u32..200)
        .map(|i| {
            (1..=6u32)
                .map(|j| {
                    let x = (i * 7919 * j) % 1009;
                    if (i + j) % 17 == 0 {
                        String::new()
                    } else if (i * j) % 23 == 0 {
                        "-99".to_string()
                    } else {
                        // Tenths from 0 to 100.8: inside the ends, rarely dyadic.
                        format!("{:?}", f64::from(x) / 10.0)
                    }
                })
                .collect()
        })
        .collect();
    let d = parsed((1..=6).map(|j| format!("p{j}")).collect(), rows, b"the pin");
    let mut sum = step(6, (1..=6).map(a).collect(), vec![a(2), a(5)]);
    sum.scale = Some(ScaleEnds {
        min: -0.5,
        max: 100.9,
    });
    sum.missing_codes = vec![-99.0];
    sum.rule = Some(MissingRule::AtLeast(4));
    let mut mean = step(7, vec![a(1), a(3), a(4), a(6)], vec![a(3)]);
    mean.scale = Some(ScaleEnds {
        min: -0.5,
        max: 100.9,
    });
    mean.missing_codes = vec![-99.0];
    mean.statistic = Some(Statistic::Mean);
    mean.rule = Some(MissingRule::AtLeast(2));
    mean.name = "the other".to_string();
    let mut every = step(8, vec![a(2), a(6)], Vec::new());
    every.missing_codes = vec![-99.0];
    every.statistic = Some(Statistic::Mean);
    every.name = "the third".to_string();
    let r = Recipe {
        steps: [sum, mean, every]
            .into_iter()
            .map(RecipeStep::ScaleScore)
            .collect(),
        ..recipe(6, step(6, Vec::new(), Vec::new()))
    };
    (d, r)
}

/// The prepared columns' bits, in order, an empty cell as the word `empty`.
fn columns_digest(p: &Prepared) -> String {
    let mut text = String::new();
    for column in 7..=9 {
        for cell in p.dataset.column_cells(ColumnAlias(column)).unwrap() {
            match cell {
                Cell::Number(v) => text.push_str(&format!("{:016x}\n", v.to_bits())),
                Cell::Missing => text.push_str("empty\n"),
                Cell::Text(_) => panic!("a score holds no text"),
            }
        }
    }
    sha256_hex(text.as_bytes())
}

/// Decision 409: exact on every platform, since the build uses only addition,
/// subtraction and division. If CI's Clean checkout turns this red, the
/// decision is Rishi's.
const PIN: &str = "43be588f8f19a1e3f8529fec07d51afdcf021bf61966179896d9f8ccfe442eab";

#[test]
fn the_prepared_columns_bits_are_pinned() {
    let (d, r) = pin_case();
    let p = build(&d, &r, CAP).unwrap();
    let scored: u64 = p
        .counts
        .iter()
        .map(|c| c.scored_from_every_item + c.scored_from_fewer)
        .sum();
    let unscored: u64 = p
        .counts
        .iter()
        .map(|c| c.not_scored_too_few + c.not_scored_overflow)
        .sum();
    assert!(
        p.counts.iter().all(|c| c.scored_from_every_item > 0),
        "{:?}",
        p.counts
    );
    assert!(scored > 300 && unscored > 0, "{:?}", p.counts);
    assert_eq!(columns_digest(&p), PIN);
}
