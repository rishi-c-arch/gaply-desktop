//! How closely the descriptive summary matches R, over
//! `tests/fixtures/stats_desc/golden.csv` (written with `/usr/local/bin/Rscript`
//! by `make_desc_fixtures.R`, which also writes the synthetic `data_*.csv`
//! files; R computed every value on the file as `read.csv` read it back).
//!
//! Each case goes through the path an operation takes: the data file's
//! bytes become a `RawTable` and a `Dataset`, a spec is built and validated,
//! and `describe` runs on it.
//!
//! THE GATE, per statistic: |gaply − R| ≤ 1e-12 · max(|R|, s), where s is
//! the case's R sd, or 1 when that sd is 0 or not defined (n < 2). n,
//! missing, each level's label, order and count, and min and max must be
//! equal; a constant column's sd must be exactly 0; and every value R gives
//! as NA must be a closed "not computable" with its code. It prints the
//! largest measured ratio |gaply − R| / max(|R|, s):
//!
//! ```text
//! cargo test -p gaply_core --test desc_r_golden -- --nocapture
//! ```

use std::collections::BTreeMap;

use gaply_core::contracts::OperationId;
use gaply_core::stats_desc::{
    describe, profile, spec, validate, Alias, ColumnRequest, ColumnResult, DescCode,
    DescribeParams, Stat, TreatAs,
};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_desc");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_desc_fixtures.R";
const GATE: f64 = 1e-12;

struct Row {
    case: String,
    data: String,
    column: String,
    treat_as: String,
    missing_codes: String,
    statistic: String,
    level: String,
    value: String,
}

fn load() -> (Vec<String>, Vec<Row>) {
    let text = std::fs::read_to_string(format!("{DIR}/golden.csv")).expect("golden.csv is present");
    let mut header = Vec::new();
    let mut rows = Vec::new();
    let mut columns_seen = false;
    for line in text.lines() {
        if let Some(h) = line.strip_prefix('#') {
            header.push(h.trim().to_string());
            continue;
        }
        if !columns_seen {
            assert_eq!(
                line, "case,data,column,treat_as,missing_codes,statistic,level,value",
                "column header"
            );
            columns_seen = true;
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 8, "row {line:?}");
        rows.push(Row {
            case: f[0].into(),
            data: f[1].into(),
            column: f[2].into(),
            treat_as: f[3].into(),
            missing_codes: f[4].into(),
            statistic: f[5].into(),
            level: f[6].into(),
            value: f[7].into(),
        });
    }
    (header, rows)
}

/// The data file: its '#' header lines, then a `Dataset` from the CSV
/// below them. The fixtures hold no quoted fields and no commas in a cell.
fn data(name: &str) -> (Vec<String>, Dataset) {
    let bytes = std::fs::read(format!("{DIR}/{name}")).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let provenance: Vec<String> = text
        .lines()
        .filter(|l| l.starts_with('#'))
        .map(str::to_string)
        .collect();
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines
        .next()
        .expect("header")
        .split(',')
        .map(str::to_string)
        .collect();
    let rows: Vec<Vec<String>> = lines
        .map(|l| l.split(',').map(str::to_string).collect())
        .collect();
    let raw = RawTable::from_rows(headers, rows);
    (
        provenance,
        build_dataset(
            raw,
            DataFingerprint::of_bytes(&bytes, "golden csv", TableRef::FirstTable),
        )
        .expect("dataset"),
    )
}

fn alias(dataset: &Dataset, header: &str) -> Alias {
    let c = dataset
        .schema
        .columns
        .iter()
        .find(|c| c.header == header)
        .unwrap_or_else(|| panic!("no column {header}"));
    Alias(c.alias)
}

fn number(s: &str) -> f64 {
    s.parse::<f64>()
        .unwrap_or_else(|e| panic!("bad number {s:?}: {e}"))
}

#[test]
fn the_descriptive_summary_matches_r_within_the_gate() {
    let (header, rows) = load();
    assert!(header.iter().any(|h| h.contains(PROVENANCE)), "{header:?}");
    let mut cases: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for r in &rows {
        cases.entry(r.case.as_str()).or_default().push(r);
    }
    assert!(
        cases.len() >= 19,
        "{} cases — the golden file is short",
        cases.len()
    );
    let files: std::collections::BTreeSet<&str> = rows.iter().map(|r| r.data.as_str()).collect();
    assert!(files.len() >= 12, "{} datasets", files.len());

    let mut worst: BTreeMap<&str, (f64, &str)> = BTreeMap::new();
    let mut compared = 0;
    for (case, rows) in &cases {
        let first = rows[0];
        let (provenance, dataset) = data(&first.data);
        assert!(
            provenance.iter().any(|h| h.contains(PROVENANCE)),
            "{}: {provenance:?}",
            first.data
        );
        let treat_as = match first.treat_as.as_str() {
            "numeric" => TreatAs::Numeric,
            "categorical" => TreatAs::Categorical,
            other => panic!("treat_as {other}"),
        };
        let missing_codes: Vec<f64> = first
            .missing_codes
            .split(';')
            .filter(|s| !s.is_empty())
            .map(number)
            .collect();
        let params = DescribeParams {
            columns: vec![ColumnRequest {
                alias: alias(&dataset, &first.column),
                treat_as,
            }],
            missing_codes,
        };
        let report = validate(&dataset, &spec(OperationId(1), &dataset, params.clone()));
        assert!(report.may_run(), "{case}: {:?}", report.errors);
        let result = describe(&dataset, &params).unwrap_or_else(|e| panic!("{case}: {e:?}"));
        let stat = |name: &str| {
            rows.iter()
                .find(|r| r.statistic == name)
                .unwrap_or_else(|| panic!("{case}: no {name}"))
        };

        match &result.columns[0] {
            ColumnResult::Numeric(s) => {
                assert_eq!(s.n.to_string(), stat("n").value, "{case}: n");
                assert_eq!(
                    s.missing.to_string(),
                    stat("missing").value,
                    "{case}: missing"
                );
                let sd_r = match stat("sd").value.as_str() {
                    "NA" => None,
                    v => Some(number(v)),
                };
                // The gate's scale: R's sd, or 1 when it is 0 or undefined.
                let scale = sd_r.filter(|v| *v != 0.0).unwrap_or(1.0);
                for (name, got) in [
                    ("mean", s.mean),
                    ("sd", s.sd),
                    ("min", s.min),
                    ("q1", s.q1),
                    ("median", s.median),
                    ("q3", s.q3),
                    ("max", s.max),
                ] {
                    let want = &stat(name).value;
                    match (got, want.as_str()) {
                        (Stat::NotComputable(code), "NA") => {
                            let expected = if s.n == 0 {
                                DescCode::NoValues
                            } else {
                                DescCode::SingleValue
                            };
                            assert_eq!(code, expected, "{case}: {name}");
                            assert!(
                                name == "sd" || s.n == 0,
                                "{case}: {name} not computable with n = {}",
                                s.n
                            );
                        }
                        (Stat::Value(g), w) if w != "NA" => {
                            let r = number(w);
                            if matches!(name, "min" | "max") {
                                assert_eq!(
                                    g.to_bits(),
                                    r.to_bits(),
                                    "{case}: {name} must be R's value exactly"
                                );
                            }
                            if name == "sd" && r == 0.0 {
                                assert_eq!(
                                    g.to_bits(),
                                    0f64.to_bits(),
                                    "{case}: a constant column's sd is exactly 0"
                                );
                            }
                            let bound = r.abs().max(scale);
                            let ratio = (g - r).abs() / bound;
                            assert!(ratio <= GATE, "{case}: {name} gaply {g:e} R {r:e}, |diff|/max(|R|, s) = {ratio:e} > {GATE:e}");
                            let entry = worst.entry(name).or_insert((0.0, ""));
                            if ratio >= entry.0 {
                                *entry = (ratio, case);
                            }
                            compared += 1;
                        }
                        (got, want) => panic!("{case}: {name} gaply {got:?}, R {want}"),
                    }
                }
            }
            ColumnResult::Categorical(s) => {
                assert_eq!(s.n.to_string(), stat("n").value, "{case}: n");
                assert_eq!(
                    s.missing.to_string(),
                    stat("missing").value,
                    "{case}: missing"
                );
                let want: Vec<(&str, &str)> = rows
                    .iter()
                    .filter(|r| r.statistic == "count")
                    .map(|r| (r.level.as_str(), r.value.as_str()))
                    .collect();
                let got: Vec<(String, String)> = s
                    .levels
                    .iter()
                    .map(|l| (l.label.clone(), l.count.to_string()))
                    .collect();
                let got: Vec<(&str, &str)> =
                    got.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
                assert_eq!(got, want, "{case}: levels, their order and counts");
                compared += 1;
            }
        }
    }
    println!("compared {compared} values over {} cases; the largest |gaply - R| / max(|R|, s) per statistic:", cases.len());
    for (name, (ratio, case)) in &worst {
        println!("  {name:<7} {ratio:.3e}  ({case})");
    }
    // 15 numeric cases × 7 statistics, less R's 8 NAs (all_missing's 7 and
    // single_row's sd), plus one level table for each of 4 categorical cases.
    assert!(compared >= 101, "{compared} values compared");
}

/// The golden values are R's answer on the numbers R read. That is the
/// answer for Gaply only if Gaply read the same numbers, so every numeric
/// cell's bits are compared with the bits R recorded (`cells_bits.csv`).
/// R's parser is not correctly rounded for every spelling; this is where
/// a cell it misread would show.
#[test]
fn gaply_parses_every_data_cell_to_the_double_r_read() {
    let text = std::fs::read_to_string(format!("{DIR}/cells_bits.csv"))
        .expect("cells_bits.csv is present");
    assert!(text
        .lines()
        .take_while(|l| l.starts_with('#'))
        .any(|h| h.contains(PROVENANCE)));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some("data,column,row,bits"));
    let mut datasets: BTreeMap<String, Dataset> = BTreeMap::new();
    let mut checked = 0;
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 4, "{line}");
        let d = datasets
            .entry(f[0].to_string())
            .or_insert_with(|| data(f[0]).1);
        let a = alias(d, f[1]);
        let row: usize = f[2].parse().expect("row");
        let cell = &d.column_cells(a.0).expect("cells")[row - 1];
        let Some(v) = cell.number() else {
            panic!("{line}: Gaply holds {cell:?} where R read a number")
        };
        assert_eq!(format!("{:016x}", v.to_bits()), f[3], "{line}: Gaply {v:e}");
        checked += 1;
    }
    assert!(checked >= 11_000, "{checked} cells checked");
}

#[test]
fn the_profile_agrees_with_r_and_reports_the_minus_99_without_applying_it() {
    let (_, rows) = load();
    let (_, coded) = data("data_coded.csv");
    let p = profile(&coded);
    let x = p.columns.iter().find(|c| c.header == "x").expect("x");
    assert_eq!(
        x.suspect_missing_codes.len(),
        1,
        "{:?}",
        x.suspect_missing_codes
    );
    assert_eq!(
        (
            x.suspect_missing_codes[0].code,
            x.suspect_missing_codes[0].cells
        ),
        (-99, 6)
    );
    let raw = |stat: &str| {
        rows.iter()
            .find(|r| r.case == "coded_raw" && r.statistic == stat)
            .expect("row")
            .value
            .clone()
    };
    assert_eq!(
        x.n.to_string(),
        raw("n"),
        "the profile counts -99 as a value"
    );
    assert_eq!(x.missing.to_string(), raw("missing"));

    // With the code listed, the validation warning goes away.
    let a = Alias(ColumnAlias(2));
    let without = DescribeParams {
        columns: vec![ColumnRequest {
            alias: a,
            treat_as: TreatAs::Numeric,
        }],
        missing_codes: vec![],
    };
    let with = DescribeParams {
        missing_codes: vec![-99.0],
        ..without.clone()
    };
    let w = validate(&coded, &spec(OperationId(1), &coded, without));
    assert_eq!(w.warnings.len(), 1);
    assert_eq!(w.warnings[0].code, DescCode::SuspectMissingCode);
    assert!(validate(&coded, &spec(OperationId(2), &coded, with))
        .warnings
        .is_empty());
}
