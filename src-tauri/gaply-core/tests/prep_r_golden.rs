//! **`gaply_core::stats_prep` against R 4.6.1 (Research OS, Phase 8M, decision
//! 409, ADR-011 Part 13), over `tests/fixtures/stats_prep/` (written by
//! `/usr/local/bin/Rscript make_prep_fixtures.R`, base R only).**
//!
//! * Which rows are scored and the four counts: exact.
//! * A sum of integer items: exact.
//! * Any other sum and every mean: within the gate below, relative to the
//!   row's rounding scale k · ε · M, M the row's sum of magnitudes, ten times
//!   the largest ratio measured, printed with the measurement (ADR-008 §7):
//!
//!   ```text
//!   cargo test -p gaply_core --test prep_r_golden -- --nocapture
//!   ```
//!
//! * A case Gaply refuses and R computes is an expected refusal, with its
//!   closed code (ADR-008 §4).
//! * Gaply parses every cell to the double R read (`cells_bits.csv`).
//!
//! The gates are asserted on every platform (decision 277).

use std::collections::BTreeMap;

use gaply_core::contracts::{CapabilityId, ValidationStatus};
use gaply_core::stats_alpha::ScaleEnds;
use gaply_core::stats_desc::{
    describe, Alias, ColumnRequest, ColumnResult, DescribeParams, Stat, TreatAs,
};
use gaply_core::stats_l1::dataset::{
    build_dataset, Cell, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_prep::{
    build, validate, MissingRule, PrepRefusal, Prepared, Recipe, RecipeStep, ScaleScoreStep,
    Statistic, CAPABILITY_ID, CAPABILITY_VERSION, ENGINE_VERSION, RECIPE_SCHEMA_VERSION,
};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_prep");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_prep_fixtures.R";
const R_VERSION: &str = "R version 4.6.1";
/// The largest table a tier admits; no golden case is near it.
const CAP: u64 = 12_800_000;

/// The gates, as a multiple of the row's rounding scale k · ε · M (ADR-011
/// Part 13), measured 2 Nov 2026, macOS aarch64, over 36 computed cases.
///
/// A sum: 1, the scale itself. The largest ratio measured is exactly 0, over
/// 1,115 non-integer rows: Gaply and R add the same doubles in the same order,
/// and R accumulates in double here (sizeof.longdouble 8). Ten times 0 is no
/// gate, so this is the rounding bound the scale states, not a measured margin.
const SUM_GATE: f64 = 1.0;
/// A mean: ten times the largest ratio measured, 6.667e-2, rev_0to10_mean row
/// 1, over 1,571 rows, rounded up at one significant figure.
const MEAN_GATE: f64 = 0.7;
/// The descriptive summary of a score against R's on that column (decision
/// 409). Its mean, minimum and maximum, relative: ten times the largest
/// measured, 1.406e-16, rev_0to10_sum mean, rounded up.
const SUMMARY_GATE: f64 = 2e-15;
/// Its SD, as a multiple of ε · |mean| / SD: an SD inherits the rows' rounding
/// at the scale of their mean (ADR-008 §6). Ten times the largest ratio
/// measured, 0.32, rev_0to10_sum, rounded up. On offset_mean it is 5.04e-9
/// relative: values near 1e9 with an SD near 1, where Gaply's row means and
/// R's differ by a last digit, and that is 0.026 of this gate.
const SUMMARY_SD_GATE: f64 = 4.0;

/// Where Gaply has no score and R has one (ADR-008 §4). Row 6 is c − c − c:
/// the total and the mean fit a double, and `stats_verify::mean`'s second pass
/// computes x − m, which does not. Gaply gives an empty cell counted as an
/// overflow; R's `rowMeans` has no second pass.
const GAPLY_OVERFLOWS_WHERE_R_COMPUTES: &[(&str, usize)] = &[("overflow_mean", 6)];

fn table(name: &str, header: &str) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(format!("{DIR}/{name}")).expect("fixture");
    let mut lines = text.lines();
    let first = lines.next().unwrap();
    assert!(first.contains(PROVENANCE), "{name}: {first}");
    let version = lines.next().unwrap();
    assert!(version.contains(R_VERSION), "{name}: {version}");
    assert!(version.contains("aarch64"), "{name}: {version}");
    assert!(version.contains("sizeof.longdouble 8"), "{name}: {version}");
    let head = lines.next().unwrap();
    assert_eq!(head, header, "{name}");
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
    statistic: Statistic,
    rule: MissingRule,
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
        "case\tfile\titems\treversed\tscale\tstatistic\trule\tmissing_codes",
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
        statistic: match f[5].as_str() {
            "sum" => Statistic::Sum,
            "mean" => Statistic::Mean,
            other => panic!("{other}"),
        },
        rule: match f[6].strip_prefix("at_least ") {
            Some(n) => MissingRule::AtLeast(n.parse().unwrap()),
            None => {
                assert_eq!(f[6], "every_item");
                MissingRule::EveryItem
            }
        },
        codes: words(&f[7]).iter().map(|v| v.parse().unwrap()).collect(),
    })
    .collect()
}

fn dataset_from(headers: Vec<String>, rows: Vec<Vec<String>>, bytes: &[u8]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(bytes, "prep golden", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// The 100,000-row table, from the generator's formula: the same IEEE
/// operations, so the same doubles, written shortest-round-trip.
fn big() -> Dataset {
    let rows = (0u64..100_000)
        .map(|i| {
            let base = ((i * 104_729) % 1009) as f64 / 50.0;
            (1..=5u64)
                .map(|j| {
                    if (i * (j + 3)) % 101 == 0 {
                        String::new()
                    } else {
                        let v = ((i * 7919 * j) % 10_007) as f64 / 100.0 + base;
                        format!("{v:?}")
                    }
                })
                .collect()
        })
        .collect();
    dataset_from(
        (1..=5).map(|j| format!("b{j}")).collect(),
        rows,
        b"big by formula",
    )
}

/// The near-overflow table, from the generator's formula.
fn overflow() -> Dataset {
    // 2^1023, 2^1022 and 2^1021, from their bits: no maths-library call.
    let a = f64::from_bits(0x7FE0_0000_0000_0000);
    let d = f64::from_bits(0x7FD0_0000_0000_0000);
    let c = 1.5 * a + f64::from_bits(0x7FC0_0000_0000_0000);
    let spell = |v: Option<f64>| v.map_or(String::new(), |v| format!("{v:?}"));
    let rows = [
        [Some(a), Some(d), None],
        [Some(c), Some(c), None],
        [Some(-a), Some(-d), None],
        [Some(-c), Some(-c), None],
        [Some(c), Some(c), Some(-c)],
        [Some(c), Some(-c), Some(-c)],
        [Some(1.0), Some(2.0), Some(3.0)],
    ]
    .iter()
    .map(|r| r.iter().map(|v| spell(*v)).collect())
    .collect();
    dataset_from(
        ["o1", "o2", "o3"].iter().map(|h| h.to_string()).collect(),
        rows,
        b"overflow by formula",
    )
}

fn data_file(file: &str) -> Dataset {
    let bytes = std::fs::read(format!("{DIR}/{file}")).expect("data file");
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

fn data(case: &Case) -> Dataset {
    if case.file != "-" {
        data_file(&case.file)
    } else if case.name.starts_with("big") {
        big()
    } else {
        assert!(case.name.starts_with("overflow"), "{}", case.name);
        overflow()
    }
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

fn recipe(case: &Case, d: &Dataset) -> Recipe {
    let k = d.schema.columns.len() as u32;
    Recipe {
        schema_version: RECIPE_SCHEMA_VERSION,
        capability_id: CapabilityId::parse(CAPABILITY_ID).unwrap(),
        capability_version: CAPABILITY_VERSION,
        engine_version: ENGINE_VERSION,
        source_columns: k,
        steps: vec![RecipeStep::ScaleScore(ScaleScoreStep {
            alias: Alias(ColumnAlias(k + 1)),
            name: "the score".to_string(),
            items: case.items.iter().map(|h| alias(d, h)).collect(),
            reversed: case.reversed.iter().map(|h| alias(d, h)).collect(),
            scale: case.scale,
            statistic: Some(case.statistic),
            rule: Some(case.rule),
            missing_codes: case.codes.clone(),
        })],
    }
}

/// A row's values as the test computes them, for the scale and for total ÷
/// count: the codes removed, a reversed item reversed. No Gaply arithmetic.
fn row_values(case: &Case, columns: &[(&[Cell], bool)], r: usize) -> Vec<f64> {
    columns
        .iter()
        .filter_map(|(cells, reversed)| match cells[r] {
            Cell::Number(v) if !case.codes.contains(&v) => Some(match (*reversed, case.scale) {
                (true, Some(e)) => e.min + e.max - v,
                _ => v,
            }),
            _ => None,
        })
        .collect()
}

#[derive(Default)]
struct Worst {
    ratio: f64,
    case: String,
    row: usize,
    rows: u64,
}

impl Worst {
    fn see(&mut self, ratio: f64, case: &str, row: usize) {
        self.rows += 1;
        if ratio > self.ratio {
            self.ratio = ratio;
            self.case = case.to_string();
            self.row = row;
        }
    }
}

fn score_of(p: &Prepared, alias: Alias, r: usize) -> Option<f64> {
    match p.dataset.column_cells(alias.0).expect("the score")[r] {
        Cell::Number(v) => Some(v),
        Cell::Missing => None,
        Cell::Text(_) => panic!("a score holds no text"),
    }
}

/// Every row of every computed case against R, the counts exact, the gates
/// asserted, the largest ratios printed.
#[test]
fn every_score_matches_r_row_by_row() {
    let expected = table("rows.tsv", "case\trow\tscore");
    let mut by_case: BTreeMap<&str, Vec<(usize, &str)>> = BTreeMap::new();
    for f in &expected {
        by_case
            .entry(&f[0])
            .or_default()
            .push((f[1].parse().unwrap(), &f[2]));
    }
    let counts: BTreeMap<String, Vec<String>> = table(
        "counts.tsv",
        "case\tevery\tfewer\ttoo_few\toverflow\tscored_rows_sum",
    )
    .into_iter()
    .map(|f| (f[0].clone(), f))
    .collect();

    let (mut worst_sum, mut worst_mean) = (Worst::default(), Worst::default());
    let (mut compared, mut exact_integer, mut exact_scale_overflow) = (0u64, 0u64, 0u64);
    let (mut mean_rows, mut mean_differs, mut mean_most_ulps) = (0u64, 0u64, 0u64);
    let (mut mean_rows_r, mut mean_differs_r) = (0u64, 0u64);
    let mut computed = 0;
    for case in cases() {
        let Some(rows) = by_case.get(case.name.as_str()) else {
            continue;
        };
        computed += 1;
        let d = data(&case);
        let r = recipe(&case, &d);
        let p = build(&d, &r, CAP).unwrap_or_else(|e| panic!("{}: {e:?}", case.name));
        let score = Alias(ColumnAlias(d.schema.columns.len() as u32 + 1));
        let k = case.items.len() as f64;
        let columns: Vec<(&[Cell], bool)> = case
            .items
            .iter()
            .map(|h| {
                (
                    d.column_cells(alias(&d, h).0).expect("item"),
                    case.reversed.contains(h),
                )
            })
            .collect();

        // Which rows, by the counts: every row of the table, the big ones too.
        let want = &counts[&case.name];
        let mut want_counts: [i64; 4] = [
            want[1].parse().unwrap(),
            want[2].parse().unwrap(),
            want[3].parse().unwrap(),
            want[4].parse().unwrap(),
        ];
        let mut want_sum: f64 = want[5].parse().unwrap();
        for (name, row) in GAPLY_OVERFLOWS_WHERE_R_COMPUTES {
            if *name == case.name {
                want_counts[0] -= 1;
                want_counts[3] += 1;
                want_sum -= *row as f64;
            }
        }
        let c = p.counts[0];
        assert_eq!(
            [
                c.scored_from_every_item as i64,
                c.scored_from_fewer as i64,
                c.not_scored_too_few as i64,
                c.not_scored_overflow as i64
            ],
            want_counts,
            "{}",
            case.name
        );
        let got_sum: f64 = (0..d.rows())
            .filter(|r| score_of(&p, score, *r).is_some())
            .map(|r| (r + 1) as f64)
            .sum();
        assert_eq!(got_sum, want_sum, "{}: which rows", case.name);

        // Total ÷ count, over every row (decision 406).
        if case.statistic == Statistic::Mean {
            for row in 0..d.rows() {
                if let Some(v) = score_of(&p, score, row) {
                    let values = row_values(&case, &columns, row);
                    let total_over_count =
                        values.iter().fold(0.0, |s, x| s + x) / values.len() as f64;
                    mean_rows += 1;
                    if v.to_bits() != total_over_count.to_bits() {
                        mean_differs += 1;
                        let ulps = v.to_bits().abs_diff(total_over_count.to_bits());
                        mean_most_ulps = mean_most_ulps.max(ulps);
                    }
                }
            }
        }

        // The values, row by row.
        for (row, text) in rows {
            let r0 = row - 1;
            let got = score_of(&p, score, r0);
            let excepted = GAPLY_OVERFLOWS_WHERE_R_COMPUTES
                .iter()
                .any(|(n, x)| *n == case.name && x == row);
            if excepted {
                assert_eq!(got, None, "{} row {row}: expected no score", case.name);
                continue;
            }
            match *text {
                "NA" | "Inf" => {
                    assert_eq!(got, None, "{} row {row}: R has {text}", case.name);
                    continue;
                }
                _ => {}
            }
            let want: f64 = text.parse().expect("a number");
            let got = got.unwrap_or_else(|| panic!("{} row {row}: no score", case.name));
            compared += 1;
            let values = row_values(&case, &columns, r0);
            let integer = values.iter().all(|v| v.fract() == 0.0);
            let m: f64 = values.iter().map(|v| v.abs()).sum();
            let scale = k * f64::EPSILON * m;
            match case.statistic {
                Statistic::Sum if integer => {
                    exact_integer += 1;
                    assert_eq!(got, want, "{} row {row}: an integer sum", case.name);
                }
                _ if !scale.is_finite() => {
                    // M itself overflows, so no ratio can be stated: exact.
                    exact_scale_overflow += 1;
                    assert_eq!(got, want, "{} row {row}: M overflows", case.name);
                }
                Statistic::Sum => {
                    let ratio = if scale == 0.0 {
                        assert_eq!(got, want, "{} row {row}", case.name);
                        0.0
                    } else {
                        (got - want).abs() / scale
                    };
                    assert!(
                        ratio <= SUM_GATE,
                        "{} row {row}: {got:e} against {want:e}, ratio {ratio:e}",
                        case.name
                    );
                    worst_sum.see(ratio, &case.name, *row);
                }
                Statistic::Mean => {
                    mean_rows_r += 1;
                    if got.to_bits() != want.to_bits() {
                        mean_differs_r += 1;
                    }
                    let ratio = if scale == 0.0 {
                        assert_eq!(got, want, "{} row {row}", case.name);
                        0.0
                    } else {
                        (got - want).abs() / scale
                    };
                    assert!(
                        ratio <= MEAN_GATE,
                        "{} row {row}: {got:e} against {want:e}, ratio {ratio:e}",
                        case.name
                    );
                    worst_mean.see(ratio, &case.name, *row);
                }
            }
        }
    }
    assert!(computed >= 30, "{computed} cases computed");
    assert!(compared >= 2_000, "{compared} values compared");
    assert!(exact_integer >= 300, "{exact_integer} integer sums");
    println!(
        "scale scores against R: {computed} cases, {compared} values: {exact_integer} integer sums exact, {exact_scale_overflow} exact where M overflows"
    );
    println!(
        "  sum:  gate {SUM_GATE:e}, {} rows, largest ratio {:.3e}, {} row {}",
        worst_sum.rows, worst_sum.ratio, worst_sum.case, worst_sum.row
    );
    println!(
        "  mean: gate {MEAN_GATE:e}, {} rows, largest ratio {:.3e}, {} row {}",
        worst_mean.rows, worst_mean.ratio, worst_mean.case, worst_mean.row
    );
    println!(
        "  means that differ from total / count: {mean_differs} of {mean_rows} rows scored, at most {mean_most_ulps} ulp; from R's rowMeans: {mean_differs_r} of {mean_rows_r} rows compared"
    );
    assert!(
        worst_mean.rows >= 1_000 && worst_sum.rows >= 1_000,
        "the gates must have seen rows: {} means, {} sums",
        worst_mean.rows,
        worst_sum.rows
    );
}

/// ADR-008 §4: where Gaply refuses and R computes, the refusal is required,
/// with its closed code.
#[test]
fn each_expected_refusal_has_its_code() {
    let refused = table("refused.tsv", "case\tcode\trows_r_scores");
    assert!(refused.len() >= 3);
    let all = cases();
    let mut r_computes = 0;
    for f in &refused {
        let case = all.iter().find(|c| c.name == f[0]).expect("a case");
        let d = data(case);
        let r = recipe(case, &d);
        let code = format!("stats.prep.{}", f[1]);
        let got = match build(&d, &r, CAP) {
            Ok(_) => panic!("{} was built", case.name),
            Err(PrepRefusal::Report(report)) => {
                assert_eq!(report.status, ValidationStatus::Invalid, "{}", case.name);
                assert_eq!(report.errors.len(), 1, "{}: {report:?}", case.name);
                assert_eq!(validate(&d, &r, CAP).errors, report.errors);
                report.errors[0].code
            }
            Err(PrepRefusal::NoRowScored { counts, .. }) => {
                assert_eq!(counts.scored_from_every_item, 0, "{}", case.name);
                gaply_core::stats_prep::PrepCode::NoRowScored
            }
            Err(PrepRefusal::Table(e)) => panic!("{}: {e:?}", case.name),
        };
        use gaply_core::contracts::codes::Closed;
        assert_eq!(got.wire(), code, "{}", case.name);
        if f[2] != "0" {
            r_computes += 1;
        }
    }
    assert!(r_computes >= 2, "{r_computes} refusals where R computes");
}

/// ADR-008 §3: a golden value is R's answer on Gaply's input only if Gaply
/// read the same numbers.
#[test]
fn gaply_parses_every_cell_to_the_double_r_read() {
    let text = std::fs::read_to_string(format!("{DIR}/cells_bits.csv")).expect("cells_bits.csv");
    let header: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(header.iter().any(|h| h.contains(PROVENANCE)));
    assert!(header
        .iter()
        .any(|h| h.contains(R_VERSION) && h.contains("sizeof.longdouble 8")));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some("data,row,column,bits"));
    let mut datasets: BTreeMap<String, Dataset> = BTreeMap::new();
    let mut checked = 0;
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 4, "{line}");
        let d = datasets
            .entry(f[0].to_string())
            .or_insert_with(|| data_file(f[0]));
        let a = alias(d, f[2]);
        let row: usize = f[1].parse().expect("row");
        let cell = &d.column_cells(a.0).expect("cells")[row - 1];
        let Some(v) = cell.number() else {
            panic!("{line}: Gaply holds {cell:?} where R read a number")
        };
        assert_eq!(format!("{:016x}", v.to_bits()), f[3], "{line}: Gaply {v:e}");
        checked += 1;
    }
    assert!(checked >= 5_000, "{checked} cells checked");
}

/// Decision 409: the descriptive summary of a score equals R's on that column.
#[test]
fn the_descriptive_summary_of_a_score_equals_rs() {
    let rows = table("summary.tsv", "case\tn\tmean\tsd\tmin\tmax");
    assert!(rows.len() >= 3);
    let all = cases();
    let (mut worst, mut worst_sd): (f64, f64) = (0.0, 0.0);
    for f in &rows {
        let case = all.iter().find(|c| c.name == f[0]).expect("a case");
        let d = data(case);
        let p = build(&d, &recipe(case, &d), CAP).expect("built");
        let score = Alias(ColumnAlias(d.schema.columns.len() as u32 + 1));
        let result = describe(
            &p.dataset,
            &DescribeParams {
                columns: vec![ColumnRequest {
                    alias: score,
                    treat_as: TreatAs::Numeric,
                }],
                missing_codes: Vec::new(),
            },
        )
        .expect("the summary runs on a score");
        let ColumnResult::Numeric(s) = &result.columns[0] else {
            panic!("a score is summarised as a number")
        };
        assert_eq!(s.n, f[1].parse::<u64>().unwrap(), "{}", case.name);
        assert_eq!(s.header, score.0.name(), "{}: the alias word", case.name);
        for (what, got, want) in [
            ("mean", s.mean, &f[2]),
            ("sd", s.sd, &f[3]),
            ("min", s.min, &f[4]),
            ("max", s.max, &f[5]),
        ] {
            let want: f64 = want.parse().unwrap();
            let Stat::Value(got) = got else {
                panic!("{} {what}: {got:?}", case.name)
            };
            let error = (got - want).abs() / want.abs().max(f64::MIN_POSITIVE);
            if what == "sd" {
                let Stat::Value(m) = s.mean else {
                    panic!("{}: no mean", case.name)
                };
                let ratio = error / (f64::EPSILON * m.abs() / want.abs());
                assert!(
                    ratio <= SUMMARY_SD_GATE,
                    "{} sd: {got:e} against {want:e}, ratio {ratio:e}",
                    case.name
                );
                worst_sd = worst_sd.max(ratio);
            } else {
                assert!(
                    error <= SUMMARY_GATE,
                    "{} {what}: {got:e} against {want:e}, {error:e}",
                    case.name
                );
                worst = worst.max(error);
            }
        }
    }
    println!(
        "the summary of a score against R: mean, min, max largest relative error {worst:.3e}, gate {SUMMARY_GATE:e}; sd largest ratio {worst_sd:.3e}, gate {SUMMARY_SD_GATE:e}"
    );
}
