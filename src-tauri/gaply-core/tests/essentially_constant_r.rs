//! **R confirms the boundary of the "essentially constant" rule (Research OS,
//! Phase 8D-1b, decision 259).** `tests/fixtures/essentially_constant/`, by
//! `make_essentially_constant.R` (`/usr/local/bin/Rscript`, R 4.6.1): one data
//! file per case, `r_outcomes.csv` (whether R's `t.test` stopped with "data
//! are essentially constant" or computed, its standard error and threshold)
//! and `cells_bits.csv` (the bits of every number R read back).
//!
//! First, Gaply must read every cell as the double R read, or the two did not
//! test the same data; the cases where R's reader misread a cell are named in
//! `r_outcomes.csv`'s header, left out of R's outcomes, and must be exactly
//! the files whose bits differ. Then, for every R case and variant: Gaply
//! refuses with the new code exactly where R stopped, and computes exactly
//! where R computed, except the paired cases where Gaply's own scale (the
//! mean of |first| and of |second|) goes beyond R's, listed in [`BEYOND_R`],
//! which R computes and Gaply refuses. Every case is at least ten times either
//! side of R's threshold (the script checks it), so a few ulps between the two
//! standard errors cannot move a case across.

use std::collections::{BTreeMap, BTreeSet};

use gaply_core::contracts::codes::Closed;
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_onesample::{
    one_sample_compute, paired_compute, OneSampleCode, OneSampleParams, PairedParams,
};
use gaply_core::stats_ttest::{compute, TTestCode, TTestParams, Variant};

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/essentially_constant"
);
const PROVENANCE: &str = "/usr/local/bin/Rscript make_essentially_constant.R";

/// The paired cases R computes and Gaply refuses: every pair differs by the
/// same small amount, and the rounding in first minus second is above
/// 10 · ε · |mean of the differences| but far below 10 · ε · the size of the
/// values.
const BEYOND_R: [&str; 3] = ["pair_tenth", "pair_300_fifth", "pair_20_hundredth"];

/// The cases R's reader misread, kept for Gaply alone: (case, the code Gaply
/// refuses with). The two paired ones are the exact-0 code, not the new one:
/// on the doubles R wrote, which Gaply reads, first − second is the same on
/// every row (−2.7000000000000002 and 1.0999999999767169, printed by R from
/// memory, 26 Oct 2026). R's reader turned one cell (3.8180000000000005,
/// 999999.60999999999 and others) into its neighbour, and that made the
/// spread R saw. The exact check comes first (decision 259).
const RUST_ONLY: [(&str, &str); 4] = [
    (
        "one_ulp_neg7_5",
        "stats.onesample.values_essentially_constant",
    ),
    ("ind_ulp_1e8", "stats.ttest.groups_essentially_constant"),
    ("pair_plus_2_7", "stats.onesample.differences_constant"),
    ("pair_1e6_minus_1_1", "stats.onesample.differences_constant"),
];

fn provenance_and_body(name: &str) -> (Vec<String>, Vec<String>) {
    let text = std::fs::read_to_string(format!("{DIR}/{name}")).expect("fixture file");
    let (head, body): (Vec<&str>, Vec<&str>) = text.lines().partition(|l| l.starts_with('#'));
    assert!(
        head.iter().any(|h| h.contains(PROVENANCE)),
        "{name}: {head:?}"
    );
    (
        head.into_iter().map(str::to_string).collect(),
        body.into_iter().map(str::to_string).collect(),
    )
}

fn data(name: &str) -> Dataset {
    let bytes = std::fs::read(format!("{DIR}/{name}")).expect("data file");
    let (_, body) = provenance_and_body(name);
    let headers: Vec<String> = body[0].split(',').map(str::to_string).collect();
    let rows = body[1..]
        .iter()
        .map(|l| l.split(',').map(str::to_string).collect())
        .collect();
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(&bytes, "essentially constant", TableRef::FirstTable),
    )
    .expect("dataset")
}

fn alias(d: &Dataset, header: &str) -> Alias {
    Alias(
        d.schema
            .columns
            .iter()
            .find(|c| c.header == header)
            .expect("column")
            .alias,
    )
}

/// What Gaply did with a case: `Ok(se)` or `Err(code)`.
fn gaply(case: &str, method: &str, variant: &str, test_value: &str) -> Result<f64, &'static str> {
    let d = data(&format!("data_{case}.csv"));
    match method {
        "one_sample" => one_sample_compute(
            &d,
            &OneSampleParams {
                outcome: alias(&d, "x"),
                test_value: Some(test_value.parse().expect("test value")),
                missing_codes: vec![],
            },
        )
        .map(|r| r.standard_error)
        .map_err(|e| <OneSampleCode as Closed>::wire(e.code)),
        "paired" => paired_compute(
            &d,
            &PairedParams {
                first: alias(&d, "a"),
                second: alias(&d, "b"),
                missing_codes: vec![],
            },
        )
        .map(|r| r.standard_error)
        .map_err(|e| <OneSampleCode as Closed>::wire(e.code)),
        "independent" => compute(
            &d,
            &TTestParams {
                outcome: alias(&d, "y"),
                group: alias(&d, "g"),
                levels: None,
                variant: Some(match variant {
                    "student" => Variant::Student,
                    "welch" => Variant::Welch,
                    other => panic!("variant {other}"),
                }),
                missing_codes: vec![],
            },
        )
        .map(|r| r.standard_error)
        .map_err(|e| <TTestCode as Closed>::wire(e.code)),
        other => panic!("method {other}"),
    }
}

fn new_code(method: &str) -> &'static str {
    match method {
        "one_sample" => "stats.onesample.values_essentially_constant",
        "paired" => "stats.onesample.differences_essentially_constant",
        _ => "stats.ttest.groups_essentially_constant",
    }
}

/// The data files whose cells Gaply reads as other doubles than R read.
fn files_read_differently() -> (BTreeSet<String>, usize) {
    let (_, body) = provenance_and_body("cells_bits.csv");
    assert_eq!(body[0], "data,column,row,bits");
    let mut datasets: BTreeMap<String, Dataset> = BTreeMap::new();
    let mut differ = BTreeSet::new();
    for line in &body[1..] {
        let f: Vec<&str> = line.split(',').collect();
        let d = datasets
            .entry(f[0].to_string())
            .or_insert_with(|| data(f[0]));
        let a = alias(d, f[1]);
        let row: usize = f[2].parse().expect("row");
        let v = d.column_cells(a.0).expect("cells")[row - 1]
            .number()
            .unwrap_or_else(|| panic!("{line}: not a number to Gaply"));
        if format!("{:016x}", v.to_bits()) != f[3] {
            differ.insert(f[0].to_string());
        }
    }
    (differ, body.len() - 1)
}

#[test]
fn gaply_reads_every_r_cases_cells_as_r_did_and_only_the_listed_cases_differ() {
    let (differ, cells) = files_read_differently();
    assert!(cells >= 150, "{cells} cells checked");
    let listed: BTreeSet<String> = RUST_ONLY
        .iter()
        .map(|(case, _)| format!("data_{case}.csv"))
        .collect();
    assert_eq!(
        differ, listed,
        "the files R misread are exactly the Rust-only cases"
    );
    let (head, _) = provenance_and_body("r_outcomes.csv");
    let named = head
        .iter()
        .find_map(|h| {
            h.strip_prefix("# Rust-only (R read back a double other than the one written): ")
        })
        .expect("the Rust-only line");
    let named: BTreeSet<&str> = named.split(' ').collect();
    assert_eq!(
        named,
        RUST_ONLY.iter().map(|(c, _)| *c).collect(),
        "r_outcomes.csv's header"
    );
}

#[test]
fn gaply_refuses_exactly_where_r_stopped_and_where_its_paired_scale_goes_beyond_r() {
    let (_, body) = provenance_and_body("r_outcomes.csv");
    assert_eq!(
        body[0],
        "case,method,variant,test_value,r_outcome,r_se,r_threshold"
    );
    let mut per_method: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut beyond = BTreeSet::new();
    let (mut stopped, mut computed) = (0, 0);
    for line in &body[1..] {
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 7, "{line}");
        let (case, method, variant, test_value, outcome) = (f[0], f[1], f[2], f[3], f[4]);
        let r_se: f64 = f[5].parse().expect("r_se");
        per_method
            .entry(method.to_string())
            .or_default()
            .insert(case.to_string());
        let got = gaply(case, method, variant, test_value);
        println!("{case} {method} {variant}: R {outcome}, Gaply {got:?}");
        match (outcome, BEYOND_R.contains(&case)) {
            ("stopped", false) => {
                assert_eq!(got, Err(new_code(method)), "{line}");
                stopped += 1;
            }
            ("computed", false) => {
                let se = got.unwrap_or_else(|c| panic!("{line}: Gaply refused {c}"));
                assert!(
                    (se - r_se).abs() <= 1e-9 * r_se,
                    "{line}: Gaply's se {se:e}"
                );
                computed += 1;
            }
            ("computed", true) => {
                assert_eq!(got, Err(new_code(method)), "{line}");
                beyond.insert(case);
            }
            (o, b) => panic!("{line}: R {o}, beyond R {b}"),
        }
    }
    for (method, cases) in &per_method {
        assert!(cases.len() >= 6, "{method}: {} cases", cases.len());
    }
    assert_eq!(per_method.len(), 3, "{per_method:?}");
    assert_eq!(
        beyond,
        BEYOND_R.into_iter().collect(),
        "every case beyond R was met"
    );
    println!(
        "{stopped} stopped and refused, {computed} computed, {} beyond R",
        beyond.len()
    );
    assert!(stopped >= 12 && computed >= 11, "{stopped} {computed}");
}

#[test]
fn the_cases_r_misread_are_refused_by_gaply_on_the_values_gaply_read() {
    for (case, code) in RUST_ONLY {
        let method = case.split('_').next().expect("prefix");
        let variants: &[&str] = if method == "ind" {
            &["student", "welch"]
        } else {
            &[""]
        };
        for variant in variants {
            let (method, test_value) = match method {
                "one" => ("one_sample", "0"),
                "pair" => ("paired", ""),
                _ => ("independent", ""),
            };
            assert_eq!(
                gaply(case, method, variant, test_value),
                Err(code),
                "{case} {variant}"
            );
        }
    }
}
