//! How closely the one-sample and paired t-tests as analyses match R
//! (Research OS, Phase 8D-1): the validation suite both registry entries name
//! (`stats.analyze.one_sample_t`, `stats.analyze.paired_t`), over
//! `tests/fixtures/stats_onesample/golden.csv` (written with
//! `/usr/local/bin/Rscript` by `make_onesample_fixtures.R`, which also writes
//! the synthetic `data_*.csv` files; R computed every value on the file as
//! `read.csv` read it back, with `t.test` and `effectsize::cohens_d`).
//!
//! Each case goes through the path an operation takes: the data file's bytes
//! become a `RawTable` and a `Dataset`, a spec is built and validated, and the
//! module's `*_compute` runs on it.
//!
//! THE GATES, per statistic, with ε = 1e-12 and every reference value R's
//! (justified in `docs/methods/one_sample_t.md` and `paired_t.md`). M is the
//! cancellation term: |mean| + |test value| for the one-sample test, |mean of
//! first| + |mean of second| for the paired test.
//!
//! | statistic | |gaply − R| at most |
//! |---|---|
//! | n, n_pairs, each exclusion count, df | 0 (equal) |
//! | se | ε · |R| |
//! | each mean, each sd, the sd of the differences | ε · max(|R|, s), s the sd that goes with it, or 1 when 0 |
//! | mean difference | ε · max(M, se) |
//! | t | ε · max(|t|, M / se) |
//! | ci bounds | ε · max(|bound|, M, se) + Q · q · se |
//! | Cohen's d, d_z | ε · max(|d|, M / s), s R's sd (of the values or of the differences) |
//! | p | P · p + P_FLOOR + 2 · f(|t|) · (t's gate) |
//!
//! Q = 1e-12 is the quantile's measured gate (`ttest_quantile.rs`); P = 1e-9
//! is the p engine's gate against R: `stats_l0::pvalue`'s at df ≤ 1e4
//! (`l0_p_reference.rs`) and `stats_dist`'s above it, the same 1e-9
//! (`stats_dist_reference.rs`, Research OS Phase 8E); P_FLOOR =
//! 1e-15 is the absolute floor; f is Student's t density at R's |t|, carrying
//! t's permitted error into p. The cancellation term is the conditioning of
//! a mean minus a test value, or of the mean of a difference of two columns
//! near 1e9: either holds about one unit in the last place of the larger
//! magnitude, ~1e-7 at 1e9, whoever computes it.
//!
//! Every case R stops on, answers NaN for, or Gaply refuses is
//! `expected_refusal`, and Gaply must refuse it with that code. It prints the
//! largest measured |gaply − R| / gate per statistic:
//!
//! ```text
//! cargo test -p gaply_core --test onesample_r_golden -- --nocapture
//! ```

use std::collections::{BTreeMap, BTreeSet};

use statrs::distribution::{Continuous, StudentsT};

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ValidationStatus};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_onesample::{
    one_sample_compute, one_sample_spec, one_sample_validate, paired_compute, paired_spec,
    paired_validate, OneSampleCode, OneSampleParams, PairedParams,
};

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/stats_onesample"
);
const PROVENANCE: &str = "/usr/local/bin/Rscript make_onesample_fixtures.R";
const EPS: f64 = 1e-12;
const Q_GATE: f64 = 1e-12;
const P_REL: f64 = 1e-9;
const P_FLOOR: f64 = 1e-15;

struct Row {
    case: String,
    data: String,
    method: String,
    test_value: String,
    missing_codes: String,
    statistic: String,
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
                line, "case,data,method,test_value,missing_codes,statistic,value",
                "column header"
            );
            columns_seen = true;
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 7, "row {line:?}");
        rows.push(Row {
            case: f[0].into(),
            data: f[1].into(),
            method: f[2].into(),
            test_value: f[3].into(),
            missing_codes: f[4].into(),
            statistic: f[5].into(),
            value: f[6].into(),
        });
    }
    (header, rows)
}

/// The data file: its '#' header lines, then a `Dataset` from the CSV below
/// them. The fixtures hold no quoted fields and no commas in a cell.
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
    assert!(
        headers == ["id", "x"] || headers == ["id", "a", "b"],
        "{name}: {headers:?}"
    );
    let width = headers.len();
    let rows: Vec<Vec<String>> = lines
        .map(|l| {
            let mut r: Vec<String> = l.split(',').map(str::to_string).collect();
            // A trailing empty field (a missing last column) splits short.
            r.resize(width, String::new());
            r
        })
        .collect();
    (
        provenance,
        build_dataset(
            RawTable::from_rows(headers, rows),
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

fn codes(first: &Row) -> Vec<f64> {
    first
        .missing_codes
        .split(';')
        .filter(|s| !s.is_empty())
        .map(number)
        .collect()
}

/// The largest |gaply − R| / gate seen per statistic, and where.
#[derive(Default)]
struct Worst(BTreeMap<&'static str, (f64, String)>);

impl Worst {
    fn check(&mut self, case: &str, name: &'static str, got: f64, want: f64, gate: f64) {
        assert!(
            gate > 0.0 && gate.is_finite(),
            "{case}: {name}: gate {gate}"
        );
        let ratio = (got - want).abs() / gate;
        assert!(
            ratio <= 1.0,
            "{case}: {name} gaply {got:e} R {want:e}: |diff| {:e} over the gate {gate:e}",
            (got - want).abs()
        );
        let e = self.0.entry(name).or_insert((0.0, String::new()));
        if ratio >= e.0 {
            *e = (ratio, case.to_string());
        }
    }
}

/// The statistics both tests share: t, df, p, the mean difference, its
/// standard error and interval, and d. `m` is the cancellation term and `s`
/// the sd d divides by, both from R.
struct Shared {
    t: f64,
    df: f64,
    p: f64,
    mean_difference: f64,
    se: f64,
    ci: [f64; 2],
    d: f64,
}

fn compare_shared(
    worst: &mut Worst,
    case: &str,
    got: Shared,
    stat: &dyn Fn(&str) -> f64,
    m: f64,
    s: f64,
    d_name: &'static str,
) {
    let (se, t, df) = (stat("se"), stat("t"), stat("df"));
    assert_eq!(got.df, df, "{case}: df is n − 1 exactly");
    worst.check(case, "se", got.se, se, EPS * se);
    worst.check(
        case,
        "mean_difference",
        got.mean_difference,
        stat("mean_difference"),
        EPS * m.max(se),
    );
    let t_gate = EPS * t.abs().max(m / se);
    worst.check(case, "t", got.t, t, t_gate);
    let q = stat("ci_upper") - stat("mean_difference");
    for (name, g, w) in [
        ("ci_lower", got.ci[0], stat("ci_lower")),
        ("ci_upper", got.ci[1], stat("ci_upper")),
    ] {
        worst.check(
            case,
            name,
            g,
            w,
            EPS * w.abs().max(m).max(se) + Q_GATE * q.abs(),
        );
    }
    let d = stat(d_name);
    worst.check(case, d_name, got.d, d, EPS * d.abs().max(m / s));
    let p_r = stat("p");
    let density = StudentsT::new(0.0, 1.0, df).expect("df").pdf(t.abs());
    worst.check(
        case,
        "p",
        got.p,
        p_r,
        P_REL * p_r + P_FLOOR + 2.0 * density * t_gate,
    );
    assert!(
        got.ci[0] <= got.mean_difference && got.mean_difference <= got.ci[1],
        "{case}"
    );
    assert!((0.0..=1.0).contains(&got.p), "{case}");
}

fn mean_and_sd(
    worst: &mut Worst,
    case: &str,
    names: [&'static str; 2],
    got: [f64; 2],
    want: [f64; 2],
) {
    let s = want[1];
    let scale = if s == 0.0 { 1.0 } else { s };
    worst.check(
        case,
        names[0],
        got[0],
        want[0],
        EPS * want[0].abs().max(scale),
    );
    worst.check(
        case,
        names[1],
        got[1],
        want[1],
        EPS * want[1].abs().max(scale),
    );
}

#[test]
fn the_one_sample_and_paired_t_tests_match_r_within_the_gates() {
    let (header, rows) = load();
    assert!(header.iter().any(|h| h.contains(PROVENANCE)), "{header:?}");
    assert!(
        header.iter().any(|h| h.contains("effectsize 1.0.3")),
        "{header:?}"
    );
    let mut cases: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for r in &rows {
        cases.entry(r.case.as_str()).or_default().push(r);
    }
    let methods: BTreeMap<&str, usize> = cases.values().fold(BTreeMap::new(), |mut m, rows| {
        *m.entry(rows[0].method.as_str()).or_default() += 1;
        m
    });
    assert!(
        methods.get("one_sample") >= Some(&14) && methods.get("paired") >= Some(&14),
        "{methods:?}: the golden file is short"
    );
    let files: BTreeSet<&str> = rows.iter().map(|r| r.data.as_str()).collect();
    assert!(files.len() >= 28, "{} datasets", files.len());

    let mut worst = Worst::default();
    let (mut computed, mut refused) = (0, 0);
    for (case, rows) in &cases {
        let first = rows[0];
        let (provenance, dataset) = data(&first.data);
        assert!(
            provenance.iter().any(|h| h.contains(PROVENANCE)),
            "{}: {provenance:?}",
            first.data
        );
        let raw = |name: &str| -> &str {
            &rows
                .iter()
                .find(|r| r.statistic == name)
                .unwrap_or_else(|| panic!("{case}: no {name}"))
                .value
        };
        let stat = |name: &str| number(raw(name));
        let refusal = rows.iter().find(|r| r.statistic == "expected_refusal");
        let count =
            |code: OneSampleCode,
             exclusions: &[gaply_core::contracts::Exclusion<OneSampleCode>]| {
                exclusions
                    .iter()
                    .find(|e| e.code == code)
                    .map_or(0, |e| e.count)
            };
        match first.method.as_str() {
            "one_sample" => {
                let test_value = number(&first.test_value);
                let p = OneSampleParams {
                    outcome: alias(&dataset, "x"),
                    test_value: Some(test_value),
                    missing_codes: codes(first),
                };
                let report = one_sample_validate(
                    &dataset,
                    &one_sample_spec(OperationId(1), &dataset, p.clone()),
                );
                let outcome = one_sample_compute(&dataset, &p);
                if let Some(row) = refusal {
                    let code = outcome
                        .expect_err(&format!("{case}: Gaply must refuse"))
                        .code;
                    assert_eq!(code.wire(), row.value, "{case}: the refusal code");
                    if code == OneSampleCode::TooFewValues {
                        assert_eq!(report.status, ValidationStatus::Invalid, "{case}");
                        assert_eq!(report.errors[0].code, code, "{case}");
                    } else {
                        assert!(
                            report.may_run(),
                            "{case}: validation passes; the arithmetic refuses"
                        );
                    }
                    refused += 1;
                    continue;
                }
                assert!(report.may_run(), "{case}: {:?}", report.errors);
                let r = outcome.unwrap_or_else(|e| panic!("{case}: {e:?}"));
                assert_eq!(r.n.to_string(), raw("n"), "{case}: n");
                for (code, name) in [
                    (OneSampleCode::MissingValue, "excluded_missing_value"),
                    (OneSampleCode::MissingCode, "excluded_missing_code"),
                ] {
                    assert_eq!(
                        count(code, &r.exclusions).to_string(),
                        raw(name),
                        "{case}: {name}"
                    );
                }
                assert_eq!(r.test_value, test_value);
                assert_eq!(
                    r.limitations,
                    [
                        OneSampleCode::ValuesNormalityNotAssessed,
                        OneSampleCode::CohensDHasNoInterval
                    ]
                );
                mean_and_sd(
                    &mut worst,
                    case,
                    ["mean", "sd"],
                    [r.mean, r.sd],
                    [stat("mean"), stat("sd")],
                );
                let m = stat("mean").abs() + test_value.abs();
                let shared = Shared {
                    t: r.t,
                    df: r.df,
                    p: r.p,
                    mean_difference: r.mean_difference,
                    se: r.standard_error,
                    ci: [r.ci_lower, r.ci_upper],
                    d: r.cohens_d,
                };
                compare_shared(&mut worst, case, shared, &stat, m, stat("sd"), "cohens_d");
            }
            "paired" => {
                let p = PairedParams {
                    first: alias(&dataset, "a"),
                    second: alias(&dataset, "b"),
                    missing_codes: codes(first),
                };
                let report =
                    paired_validate(&dataset, &paired_spec(OperationId(1), &dataset, p.clone()));
                let outcome = paired_compute(&dataset, &p);
                if let Some(row) = refusal {
                    let code = outcome
                        .expect_err(&format!("{case}: Gaply must refuse"))
                        .code;
                    assert_eq!(code.wire(), row.value, "{case}: the refusal code");
                    if code == OneSampleCode::TooFewPairs {
                        assert_eq!(report.status, ValidationStatus::Invalid, "{case}");
                        assert_eq!(report.errors[0].code, code, "{case}");
                    } else {
                        assert!(
                            report.may_run(),
                            "{case}: validation passes; the arithmetic refuses"
                        );
                    }
                    refused += 1;
                    continue;
                }
                assert!(report.may_run(), "{case}: {:?}", report.errors);
                let r = outcome.unwrap_or_else(|e| panic!("{case}: {e:?}"));
                assert_eq!(r.n_pairs.to_string(), raw("n_pairs"), "{case}: n_pairs");
                for (code, name) in [
                    (OneSampleCode::MissingFirst, "excluded_missing_first"),
                    (OneSampleCode::MissingSecond, "excluded_missing_second"),
                    (OneSampleCode::MissingCode, "excluded_missing_code"),
                ] {
                    assert_eq!(
                        count(code, &r.exclusions).to_string(),
                        raw(name),
                        "{case}: {name}"
                    );
                }
                assert_eq!(
                    r.limitations,
                    [
                        OneSampleCode::DifferencesNormalityNotAssessed,
                        OneSampleCode::CohensDHasNoInterval
                    ]
                );
                mean_and_sd(
                    &mut worst,
                    case,
                    ["mean_first", "sd_first"],
                    [r.first.mean, r.first.sd],
                    [stat("mean_first"), stat("sd_first")],
                );
                mean_and_sd(
                    &mut worst,
                    case,
                    ["mean_second", "sd_second"],
                    [r.second.mean, r.second.sd],
                    [stat("mean_second"), stat("sd_second")],
                );
                let sd_d = stat("sd_differences");
                worst.check(case, "sd_differences", r.sd_differences, sd_d, EPS * sd_d);
                let m = stat("mean_first").abs() + stat("mean_second").abs();
                let shared = Shared {
                    t: r.t,
                    df: r.df,
                    p: r.p,
                    mean_difference: r.mean_difference,
                    se: r.standard_error,
                    ci: [r.ci_lower, r.ci_upper],
                    d: r.cohens_dz,
                };
                compare_shared(&mut worst, case, shared, &stat, m, sd_d, "cohens_dz");
            }
            other => panic!("{case}: method {other}"),
        }
        computed += 1;
    }
    println!(
        "{computed} computed and {refused} refused cases; the largest |gaply - R| / gate per statistic:"
    );
    for (name, (ratio, case)) in &worst.0 {
        println!("  {name:<16} {ratio:.3e}  ({case})");
    }
    // 17 one-sample and 15 paired cases, of which 2 and 3 refuse: the two at
    // df 10,001 compute since 8E.
    assert_eq!((computed, refused), (27, 5), "cases compared");
}

/// The golden values are R's answer on the numbers R read; that is the answer
/// for Gaply only if Gaply read the same numbers (`cells_bits.csv`).
#[test]
fn gaply_parses_every_cell_to_the_double_r_read() {
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
    assert!(checked >= 15_000, "{checked} cells checked");
}

/// The -99 in `data_one_coded.csv` and `data_paired_coded.csv` is reported,
/// not applied, until the user ticks it; ticking it removes the warning and
/// excludes exactly those rows.
#[test]
fn a_candidate_missing_code_is_a_warning_until_ticked() {
    let (_, d) = data("data_one_coded.csv");
    let mut p = OneSampleParams {
        outcome: alias(&d, "x"),
        test_value: Some(30.0),
        missing_codes: vec![],
    };
    let w = one_sample_validate(&d, &one_sample_spec(OperationId(1), &d, p.clone()));
    assert_eq!(w.status, ValidationStatus::ValidWithWarnings);
    assert_eq!(w.warnings.len(), 1);
    assert_eq!(w.warnings[0].code, OneSampleCode::SuspectMissingCode);
    p.missing_codes = vec![-99.0];
    let v = one_sample_validate(&d, &one_sample_spec(OperationId(2), &d, p.clone()));
    assert_eq!(v.status, ValidationStatus::Valid, "{:?}", v.warnings);

    let (_, d) = data("data_paired_coded.csv");
    let mut p = PairedParams {
        first: alias(&d, "a"),
        second: alias(&d, "b"),
        missing_codes: vec![],
    };
    let w = paired_validate(&d, &paired_spec(OperationId(1), &d, p.clone()));
    assert_eq!(w.status, ValidationStatus::ValidWithWarnings);
    assert_eq!(w.warnings.len(), 2, "one per column");
    p.missing_codes = vec![-99.0];
    let v = paired_validate(&d, &paired_spec(OperationId(2), &d, p.clone()));
    assert_eq!(v.status, ValidationStatus::Valid, "{:?}", v.warnings);
    let r = paired_compute(&d, &p).expect("computes");
    let excluded = |code| {
        r.exclusions
            .iter()
            .find(|e| e.code == code)
            .map_or(0, |e| e.count)
    };
    // Rows 2, 9, 17, 25 and 30 hold a -99: five rows, the one with two counted once.
    assert_eq!(excluded(OneSampleCode::MissingCode), 5);
}
