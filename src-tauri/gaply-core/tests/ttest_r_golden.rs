//! How closely the t-test as an analysis matches R, over
//! `tests/fixtures/stats_ttest/golden.csv` (written with `/usr/local/bin/Rscript`
//! by `make_ttest_fixtures.R`, which also writes the synthetic `data_*.csv`
//! files; R computed every value on the file as `read.csv` read it back, with
//! `t.test` and `effectsize::cohens_d`).
//!
//! Each case goes through the path an operation takes: the data file's bytes
//! become a `RawTable` and a `Dataset`, a spec is built and validated, and
//! `compute` runs on it, once per variant.
//!
//! THE GATES, per statistic, with ε = 1e-12, M = |mean1| + |mean2| and every
//! reference value R's (they are justified in `docs/methods/independent_t.md`):
//!
//! | statistic | |gaply − R| at most |
//! |---|---|
//! | n1, n2, each exclusion count, the level count | 0 (equal) |
//! | df, Student | 0 (equal) |
//! | df, Welch; se; sd ratio | ε · |R| |
//! | mean1, mean2, sd1, sd2 | ε · max(|R|, s), s the group's R sd, or 1 when 0 |
//! | mean difference | ε · max(M, se) |
//! | t | ε · max(|t|, M / se) |
//! | ci bounds | ε · max(|bound|, M, se) + Q · q · se |
//! | Cohen's d | ε · max(|d|, M / s_d), s_d the standardizer from R's sds |
//! | p | P · p + P_FLOOR + 2 · f(|t|) · (t's gate) |
//!
//! Q = 1e-12 is the quantile's own measured gate (`ttest_quantile.rs`); P =
//! 1e-9 is the p engine's gate against R: `stats_l0::pvalue`'s at df ≤ 1e4
//! (`l0_p_reference.rs`) and `stats_dist`'s above it, the same 1e-9
//! (`stats_dist_reference.rs`, Research OS Phase 8E); P_FLOOR
//! = 1e-15 is the absolute floor, so a tiny p cannot pass or fail on its
//! relative error alone; f is Student's t density at R's |t|, carrying t's
//! permitted error into p. The cancellation terms (M) are the conditioning
//! of a difference of means: values near 1e9 hold their means to about one
//! unit in the last place, ~1e-7, whoever sums them.
//!
//! A constant group's sd must be exactly 0. Every case R stops on or Gaply
//! refuses is `expected_refusal`, and Gaply must refuse it with that code. It
//! prints the largest measured |gaply − R| / gate per statistic:
//!
//! ```text
//! cargo test -p gaply_core --test ttest_r_golden -- --nocapture
//! ```

use std::collections::{BTreeMap, BTreeSet};

use statrs::distribution::{Continuous, StudentsT};

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ValidationStatus};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_ttest::{
    compute, spec, validate, Standardizer, Stat, TTestCode, TTestParams, TTestResult, Variant,
};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_ttest");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_ttest_fixtures.R";
const EPS: f64 = 1e-12;
const Q_GATE: f64 = 1e-12;
const P_REL: f64 = 1e-9;
const P_FLOOR: f64 = 1e-15;

struct Row {
    case: String,
    data: String,
    l1: u32,
    l2: u32,
    missing_codes: String,
    variant: String,
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
                line, "case,data,level_index1,level_index2,missing_codes,variant,statistic,value",
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
            l1: f[2].parse().expect("index"),
            l2: f[3].parse().expect("index"),
            missing_codes: f[4].into(),
            variant: f[5].into(),
            statistic: f[6].into(),
            value: f[7].into(),
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
    assert_eq!(headers, ["id", "y", "g"], "{name}");
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

fn params(d: &Dataset, first: &Row, variant: Variant) -> TTestParams {
    TTestParams {
        outcome: alias(d, "y"),
        group: alias(d, "g"),
        levels: Some([first.l1, first.l2]),
        variant: Some(variant),
        missing_codes: first
            .missing_codes
            .split(';')
            .filter(|s| !s.is_empty())
            .map(number)
            .collect(),
    }
}

fn excluded(r: &TTestResult, code: TTestCode) -> u64 {
    r.exclusions
        .iter()
        .find(|e| e.code == code)
        .map_or(0, |e| e.count)
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

#[test]
fn the_t_test_matches_r_within_the_gates() {
    let (header, rows) = load();
    assert!(header.iter().any(|h| h.contains(PROVENANCE)), "{header:?}");
    assert!(
        header.iter().any(|h| h.contains("effectsize 1.0.3")),
        "{header:?}"
    );
    let mut cases: BTreeMap<(&str, &str), Vec<&Row>> = BTreeMap::new();
    for r in &rows {
        cases
            .entry((r.case.as_str(), r.variant.as_str()))
            .or_default()
            .push(r);
    }
    let names: BTreeSet<&str> = rows.iter().map(|r| r.case.as_str()).collect();
    assert!(
        names.len() >= 14,
        "{} cases: the golden file is short",
        names.len()
    );
    let files: BTreeSet<&str> = rows.iter().map(|r| r.data.as_str()).collect();
    assert!(files.len() >= 14, "{} datasets", files.len());

    let mut worst = Worst::default();
    let mut computed = 0;
    let mut refused = 0;
    for ((case, variant_word), rows) in &cases {
        let first = rows[0];
        let (provenance, dataset) = data(&first.data);
        assert!(
            provenance.iter().any(|h| h.contains(PROVENANCE)),
            "{}: {provenance:?}",
            first.data
        );
        let variant = match *variant_word {
            "student" => Variant::Student,
            "welch" => Variant::Welch,
            other => panic!("variant {other}"),
        };
        let stat = |name: &str| -> &str {
            &rows
                .iter()
                .find(|r| r.statistic == name)
                .unwrap_or_else(|| panic!("{case} {variant_word}: no {name}"))
                .value
        };
        let p = params(&dataset, first, variant);
        let report = validate(&dataset, &spec(OperationId(1), &dataset, p.clone()));
        let outcome = compute(&dataset, &p);

        if let Some(row) = rows.iter().find(|r| r.statistic == "expected_refusal") {
            let code = outcome
                .expect_err(&format!("{case} {variant_word}: Gaply must refuse"))
                .code;
            assert_eq!(
                code.wire(),
                row.value,
                "{case} {variant_word}: the refusal code"
            );
            if code == TTestCode::GroupTooSmall {
                assert_eq!(
                    report.status,
                    ValidationStatus::Invalid,
                    "{case}: refused at validation"
                );
                assert_eq!(report.errors[0].code, TTestCode::GroupTooSmall);
            } else {
                assert!(
                    report.may_run(),
                    "{case}: validation passes; the arithmetic refuses"
                );
            }
            refused += 1;
            continue;
        }
        assert!(
            report.may_run(),
            "{case} {variant_word}: {:?}",
            report.errors
        );
        let r = outcome.unwrap_or_else(|e| panic!("{case} {variant_word}: {e:?}"));
        assert_eq!(r.variant, variant);

        // Counts are exact.
        assert_eq!(r.groups[0].n.to_string(), stat("n1"), "{case}: n1");
        assert_eq!(r.groups[1].n.to_string(), stat("n2"), "{case}: n2");
        for (code, name) in [
            (TTestCode::MissingGroup, "excluded_missing_group"),
            (TTestCode::OtherLevel, "excluded_other_level"),
            (TTestCode::MissingOutcome, "excluded_missing_outcome"),
            (TTestCode::MissingCode, "excluded_missing_code"),
        ] {
            assert_eq!(excluded(&r, code).to_string(), stat(name), "{case}: {name}");
        }
        assert_eq!(
            [r.groups[0].level, r.groups[1].level],
            [first.l1, first.l2],
            "{case}: the levels named"
        );
        assert!(
            r.limitations.contains(&TTestCode::NormalityNotAssessed)
                && r.limitations.contains(&TTestCode::CohensDHasNoInterval)
        );

        let m1 = number(stat("mean1"));
        let m2 = number(stat("mean2"));
        let s1 = number(stat("sd1"));
        let s2 = number(stat("sd2"));
        let se = number(stat("se"));
        let t = number(stat("t"));
        let df = number(stat("df"));
        let big_m = m1.abs() + m2.abs();
        for (name, got, want, s) in [
            ("mean1", r.groups[0].mean, m1, s1),
            ("mean2", r.groups[1].mean, m2, s2),
            ("sd1", r.groups[0].sd, s1, s1),
            ("sd2", r.groups[1].sd, s2, s2),
        ] {
            let scale = if s == 0.0 { 1.0 } else { s };
            worst.check(case, name, got, want, EPS * want.abs().max(scale));
        }
        for (k, s) in [(0, s1), (1, s2)] {
            if s == 0.0 {
                assert_eq!(
                    r.groups[k].sd.to_bits(),
                    0f64.to_bits(),
                    "{case}: a constant group's sd is exactly 0"
                );
            }
        }
        match variant {
            Variant::Student => {
                assert_eq!(r.df, df, "{case}: Student's df is n1 + n2 - 2 exactly");
                assert_eq!(r.d_standardizer, Standardizer::PooledSd);
            }
            Variant::Welch => {
                worst.check(case, "df_welch", r.df, df, EPS * df.abs());
                assert_eq!(r.d_standardizer, Standardizer::RootMeanVariance);
            }
        }
        worst.check(case, "se", r.standard_error, se, EPS * se);
        worst.check(
            case,
            "mean_difference",
            r.mean_difference,
            number(stat("mean_difference")),
            EPS * big_m.max(se),
        );
        let t_gate = EPS * t.abs().max(big_m / se);
        worst.check(case, "t", r.t, t, t_gate);
        let q = number(stat("ci_upper")) - number(stat("mean_difference"));
        for (name, got, want) in [
            ("ci_lower", r.ci_lower, number(stat("ci_lower"))),
            ("ci_upper", r.ci_upper, number(stat("ci_upper"))),
        ] {
            worst.check(
                case,
                name,
                got,
                want,
                EPS * want.abs().max(big_m).max(se) + Q_GATE * q.abs(),
            );
        }
        let (n1, n2) = (r.groups[0].n as f64, r.groups[1].n as f64);
        let s_d = match variant {
            Variant::Student => {
                (((n1 - 1.0) * s1 * s1 + (n2 - 1.0) * s2 * s2) / (n1 + n2 - 2.0)).sqrt()
            }
            Variant::Welch => ((s1 * s1 + s2 * s2) / 2.0).sqrt(),
        };
        let d = number(stat("cohens_d"));
        worst.check(
            case,
            "cohens_d",
            r.cohens_d,
            d,
            EPS * d.abs().max(big_m / s_d),
        );
        let p_r = number(stat("p"));
        let density = StudentsT::new(0.0, 1.0, df).expect("df").pdf(t.abs());
        worst.check(
            case,
            "p",
            r.p,
            p_r,
            P_REL * p_r + P_FLOOR + 2.0 * density * t_gate,
        );
        match (r.sd_ratio, stat("sd_ratio")) {
            (Stat::Value(g), w) if w != "NA" => {
                let w = number(w);
                worst.check(case, "sd_ratio", g, w, EPS * w);
            }
            (Stat::NotComputable(code), "NA") => {
                assert_eq!(code, TTestCode::SdRatioUndefined, "{case}")
            }
            (got, want) => panic!("{case}: sd_ratio gaply {got:?}, R {want}"),
        }
        assert!(r.ci_lower <= r.mean_difference && r.mean_difference <= r.ci_upper);
        computed += 1;
    }
    println!(
        "{computed} computed and {refused} refused (case, variant) pairs over {} cases; the largest |gaply - R| / gate per statistic:",
        names.len()
    );
    for (name, (ratio, case)) in &worst.0 {
        println!("  {name:<16} {ratio:.3e}  ({case})");
    }
    // 19 cases × 2 variants (large_df, above df 1e4, since 8E), of which
    // both_constant and group_of_one refuse.
    assert_eq!((computed, refused), (34, 4), "pairs compared");
}

/// The golden values are R's answer on the numbers R read; that is the answer
/// for Gaply only if Gaply read the same numbers (`cells_bits.csv`).
#[test]
fn gaply_parses_every_outcome_cell_to_the_double_r_read() {
    let text = std::fs::read_to_string(format!("{DIR}/cells_bits.csv"))
        .expect("cells_bits.csv is present");
    assert!(text
        .lines()
        .take_while(|l| l.starts_with('#'))
        .any(|h| h.contains(PROVENANCE)));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    assert_eq!(lines.next(), Some("data,row,bits"));
    let mut datasets: BTreeMap<String, Dataset> = BTreeMap::new();
    let mut checked = 0;
    for line in lines {
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 3, "{line}");
        let d = datasets
            .entry(f[0].to_string())
            .or_insert_with(|| data(f[0]).1);
        let a = alias(d, "y");
        let row: usize = f[1].parse().expect("row");
        let cell = &d.column_cells(a.0).expect("cells")[row - 1];
        let Some(v) = cell.number() else {
            panic!("{line}: Gaply holds {cell:?} where R read a number")
        };
        assert_eq!(format!("{:016x}", v.to_bits()), f[2], "{line}: Gaply {v:e}");
        checked += 1;
    }
    assert!(checked >= 5_000, "{checked} cells checked");
}

/// The -99 in `data_coded.csv` is reported, not applied, until the user ticks
/// it; ticking it removes the warning and excludes exactly those rows.
#[test]
fn a_candidate_missing_code_in_the_outcome_is_a_warning_until_ticked() {
    let (_, d) = data("data_coded.csv");
    let mut p = TTestParams {
        outcome: alias(&d, "y"),
        group: alias(&d, "g"),
        levels: None,
        variant: Some(Variant::Welch),
        missing_codes: vec![],
    };
    let w = validate(&d, &spec(OperationId(1), &d, p.clone()));
    assert_eq!(w.status, ValidationStatus::ValidWithWarnings);
    assert_eq!(w.warnings.len(), 1);
    assert_eq!(w.warnings[0].code, TTestCode::SuspectMissingCode);
    p.missing_codes = vec![-99.0];
    let v = validate(&d, &spec(OperationId(2), &d, p.clone()));
    assert_eq!(v.status, ValidationStatus::Valid, "{:?}", v.warnings);
    let r = compute(&d, &p).expect("computes");
    // Row 9 holds -99 and an empty group: counted once, as missing_group.
    assert_eq!(excluded(&r, TTestCode::MissingCode), 4);
    assert_eq!(excluded(&r, TTestCode::MissingGroup), 1);
}

/// Decision 91 (the Phase 4 evaluation's F2): the offset sentence in ADR-008
/// and on the method page states what was measured. R's `t.test` and
/// `effectsize::cohens_d` agree on `data_offset.csv` (2.2e-16 Student, 0
/// Welch); the 1.2e-8 is between a one-pass mean, `sum(x)/n`, and R's `mean`.
/// Both documents must say so, name the R call, and never repeat the old claim.
#[test]
fn the_offset_sentence_states_the_measured_comparison() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/");
    for doc in [
        "adr/ADR-008-statistical-reference-strategy.md",
        "methods/independent_t.md",
    ] {
        let text = std::fs::read_to_string(format!("{root}{doc}"))
            .unwrap_or_else(|e| panic!("{doc}: {e}"));
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            !flat.contains("disagree with each other by 1.2e-8"),
            "{doc} still attributes 1.2e-8 to R against R"
        );
        assert!(flat.contains("sum(x)/n"), "{doc} names the one-pass mean");
        assert!(
            flat.contains("1.194e-8") && flat.contains("1.200e-8"),
            "{doc} quotes the measured differences"
        );
        assert!(
            flat.contains("effectsize::cohens_d"),
            "{doc} names the R call"
        );
    }
}
