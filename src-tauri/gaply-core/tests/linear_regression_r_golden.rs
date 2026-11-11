//! **`gaply_core::stats_linear_regression` against R 4.6.1 (Research OS, Phase
//! 9A-1, decisions 669 and 671), over `tests/fixtures/stats_linear_regression/`
//! (written by `/usr/local/bin/Rscript make_linear_regression_fixtures.R`).**
//!
//! Every number of every case is compared with R's, and each family has its gate,
//! set at ten times the largest error measured (decision 316), printed with the
//! measurement:
//!
//! ```text
//! cargo test -p gaply_core --test linear_regression_r_golden -- --nocapture
//! ```
//!
//! R's oracles: `lm` and `summary.lm` (coefficients, standard errors, t, p, σ, R²,
//! adjusted R², F), `anova` (the sums of squares), `confint` (the intervals), `pf`,
//! `cor`, `fitted`, `resid`, `mean`, `sd`. The standardized coefficients, VIF and
//! tolerance, the partial and part correlations, the Durbin-Watson statistic and
//! the residuals' summary are written in R by a different route from Gaply's (the
//! generator's header says which). Counts, df, exclusions and the note are
//! compared exactly.
//!
//! The families: every p by a pure relative error; every coefficient-scale number
//! relative below a floor of 1e-12, as `related_ranks_r_golden.rs` does; every
//! residual-scale number (σ, the residual SS and mean square, the residuals'
//! summary, the standardized residuals, the Durbin-Watson statistic) against the
//! residuals' own scale, never a fixed floor, the residuals' mean as
//! |Gaply − R| / sd(residuals). The well-conditioned cases and the ill-conditioned
//! ones (`ILL_CONDITIONED`) have gates of their own, since an ill-conditioned
//! model's error grows with its condition number.
//!
//! In `essentially_perfect` only the coefficients, R², the note and the
//! not-computable codes are compared (`PERFECT_FIT_KEYS`): its residual-scale
//! numbers are rounding noise in R, and so is every number built on them.

use std::collections::BTreeMap;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{OperationId, ParamKey, ParamValue};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_linear_regression::{
    compute, spec, validate, LinearRegressionCode, LinearRegressionOutcome, LinearRegressionParams,
    Stat,
};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_linear_regression");
const PROVENANCE: &str = "/usr/local/bin/Rscript make_linear_regression_fixtures.R";
const R_VERSION: &str = "R version 4.6.1";

/// The cases whose models are ill-conditioned: offsets, a predictor near its own
/// mean, and two pairs of nearly collinear predictors.
const ILL_CONDITIONED: &[&str] = &["offsets", "tolerance", "correlated_999", "correlated_99999"];

/// The gates, each ten times the largest error measured over every case of its
/// tier (the measurement beside it, 11 Nov 2026, macOS aarch64), rounded up.
const GATES: &[(&str, f64)] = &[
    ("p", 8.1e-10),        // 8.083e-11, big_49 predictor.36.p, pure relative
    ("coef", 6e-11),       // 5.944e-12, big_49 predictor.37.partial
    ("resid", 8.9e-14),    // 8.855e-15, big_49 residuals.residual.maximum, against sd(residuals)
    ("p_ill", 6.3e-8),     // 6.201e-9, tolerance predictor.1.p, pure relative
    ("coef_ill", 2.6e-9),  // 2.551e-10, offsets residuals.standardized_predicted.maximum
    ("resid_ill", 6.2e-9), // 6.123e-10, tolerance residuals.residual.maximum, against sd(residuals)
    ("perfect", 5.6e-15),  // 5.551e-16, essentially_perfect predictor.0.b, against the largest |b|
];

/// The keys of `essentially_perfect` that are compared (decision 671); every other
/// key is checked to exist and left out, as the module doc says. Its coefficients
/// are measured against the case's largest |b|, since a coefficient whose value is
/// 0 is rounding noise in both R and Gaply (measured 3.016e-6 relative below the
/// 1e-12 floor, 5.551e-16 against the largest |b|), in a tier of their own.
fn perfect_fit_compared(key: &str) -> bool {
    key.ends_with(".b") || key == "model.r_squared" || key.starts_with("note.") || key == "rows_used"
}

/// The keys compared exactly: whole numbers.
fn exact(key: &str) -> bool {
    key.starts_with("excluded.") || key.starts_with("note.") || key.ends_with(".df") || key == "rows_used"
}

/// A residual-scale key and how its error is scaled: `Own` relative to R's value,
/// `ResidualSd` against R's sd of the residuals.
enum Scale {
    Own,
    ResidualSd,
}

fn residual_scale(key: &str) -> Option<Scale> {
    match key {
        "anova.residual.sum_of_squares"
        | "anova.residual.mean_square"
        | "model.durbin_watson"
        | "residuals.standardized_residual.minimum"
        | "residuals.standardized_residual.maximum" => Some(Scale::Own),
        "model.standard_error_of_estimate"
        | "residuals.residual.minimum"
        | "residuals.residual.maximum"
        | "residuals.residual.mean"
        | "residuals.residual.standard_deviation" => Some(Scale::ResidualSd),
        _ => None,
    }
}

fn family(key: &str) -> &'static str {
    if key.ends_with(".p") {
        "p"
    } else if residual_scale(key).is_some() {
        "resid"
    } else {
        "coef"
    }
}

struct Case {
    name: String,
    file: String,
    outcome: String,
    predictors: Vec<String>,
    missing_codes: Vec<f64>,
}

fn table(name: &str) -> Vec<Vec<String>> {
    let text =
        std::fs::read_to_string(format!("{DIR}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"));
    let stamp: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(stamp.iter().any(|l| l.contains(PROVENANCE)), "{name}: not written by {PROVENANCE}");
    assert!(stamp.iter().any(|l| l.contains(R_VERSION)), "{name}: not written with {R_VERSION}");
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|l| l.split('\t').map(str::to_string).collect())
        .collect()
}

fn words(f: &str) -> Vec<String> {
    if f == "-" {
        Vec::new()
    } else {
        f.split(' ').map(str::to_string).collect()
    }
}

fn cases() -> Vec<Case> {
    table("cases.tsv")
        .into_iter()
        .map(|f| Case {
            name: f[0].clone(),
            file: f[1].clone(),
            outcome: f[2].clone(),
            predictors: words(&f[3]),
            missing_codes: words(&f[4]).iter().map(|v| v.parse().unwrap()).collect(),
        })
        .collect()
}

fn expected() -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for f in table("expected.tsv") {
        out.entry(f[0].clone()).or_default().insert(f[1].clone(), f[2].clone());
    }
    out
}

fn dataset_from(headers: Vec<String>, rows: Vec<Vec<String>>, bytes: &[u8]) -> Dataset {
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(bytes, "linear regression golden", TableRef::FirstTable),
    )
    .expect("dataset")
}

/// The generator's 100,000-row formula, the same IEEE operations: integer
/// arithmetic and one division per column.
fn big(k: usize) -> Dataset {
    let mut primes = Vec::new();
    let mut p = 101u64;
    while primes.len() < k {
        if (2..=((p as f64).sqrt() as u64)).all(|d| p % d != 0) {
            primes.push(p);
        }
        p += 1;
    }
    let rows = (0u64..100_000)
        .map(|i| {
            let ints: Vec<i64> = (0..k).map(|j| ((i * primes[j]) % (1009 + 2 * (j as u64 + 1))) as i64).collect();
            let cj = |j: usize| ((j as i64 + 1) % 7) - 3;
            let y_int: i64 = ints.iter().enumerate().map(|(j, v)| cj(j) * v).sum::<i64>() + ((i * 104729) % 10007) as i64;
            let y = y_int as f64 / 100.0;
            let mut row = vec![(i + 1).to_string(), format!("{y:?}")];
            for (j, v) in ints.iter().enumerate() {
                let x = *v as f64 / (10 + j + 1) as f64;
                row.push(format!("{x:?}"));
            }
            row
        })
        .collect();
    let mut headers = vec!["id".to_string(), "y".to_string()];
    headers.extend((1..=k).map(|j| format!("x{j}")));
    dataset_from(headers, rows, format!("big by formula {k}").as_bytes())
}

fn data(case: &Case) -> Dataset {
    if case.file == "-" {
        return big(case.predictors.len());
    }
    let bytes = std::fs::read(format!("{DIR}/{}", case.file)).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().expect("header").split(',').map(str::to_string).collect();
    // Empty trailing cells are kept: split gives them.
    let rows = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    dataset_from(headers, rows, &bytes)
}

fn alias(d: &Dataset, header: &str) -> Alias {
    match d.schema.columns.iter().find(|c| c.header == header) {
        Some(c) => Alias(c.alias),
        // A column the dataset does not have: the refusal case's.
        None => Alias(ColumnAlias(99)),
    }
}

enum Got {
    Number(f64),
    Nc(String),
}

fn stat(s: Stat) -> Got {
    match s {
        Stat::Value(v) => Got::Number(v),
        Stat::NotComputable(c) => Got::Nc(c.wire().to_string()),
    }
}

/// Every number of a result, keyed as the generator keys R's.
fn keys(r: &LinearRegressionOutcome) -> BTreeMap<String, Got> {
    let mut m = BTreeMap::new();
    let mut put = |k: String, g: Got| {
        assert!(m.insert(k.clone(), g).is_none(), "{k} twice");
    };
    put("rows_used".into(), Got::Number(r.rows_used as f64));
    for (i, c) in r.columns.iter().enumerate() {
        put(format!("column.{i}.mean"), stat(c.mean));
        put(format!("column.{i}.sd"), stat(c.sd));
    }
    let md = &r.model;
    put("model.r".into(), stat(md.r));
    put("model.r_squared".into(), stat(md.r_squared));
    put("model.adjusted_r_squared".into(), stat(md.adjusted_r_squared));
    put("model.standard_error_of_estimate".into(), stat(md.standard_error_of_estimate));
    put("model.durbin_watson".into(), stat(md.durbin_watson));
    let a = &r.anova;
    for (name, s) in [("regression", &a.regression), ("residual", &a.residual)] {
        put(format!("anova.{name}.sum_of_squares"), stat(s.sum_of_squares));
        put(format!("anova.{name}.df"), Got::Number(s.df as f64));
        put(format!("anova.{name}.mean_square"), stat(s.mean_square));
    }
    put("anova.total.sum_of_squares".into(), stat(a.total.sum_of_squares));
    put("anova.total.df".into(), Got::Number(a.total.df as f64));
    put("anova.f".into(), stat(a.f));
    put("anova.p".into(), stat(a.p));
    let c = &r.intercept;
    for (name, s) in [("b", c.b), ("standard_error", c.standard_error), ("t", c.t), ("p", c.p), ("lower", c.lower), ("upper", c.upper)] {
        put(format!("intercept.{name}"), stat(s));
    }
    for (j, p) in r.predictors.iter().enumerate() {
        for (name, s) in [
            ("b", p.b),
            ("standard_error", p.standard_error),
            ("beta", p.beta),
            ("t", p.t),
            ("p", p.p),
            ("lower", p.lower),
            ("upper", p.upper),
            ("zero_order", p.zero_order),
            ("partial", p.partial),
            ("part", p.part),
            ("tolerance", p.tolerance),
            ("vif", p.vif),
        ] {
            put(format!("predictor.{j}.{name}"), stat(s));
        }
    }
    let rs = &r.residuals;
    for (name, s) in [("predicted", &rs.predicted), ("residual", &rs.residual)] {
        put(format!("residuals.{name}.minimum"), stat(s.minimum));
        put(format!("residuals.{name}.maximum"), stat(s.maximum));
        put(format!("residuals.{name}.mean"), stat(s.mean));
        put(format!("residuals.{name}.standard_deviation"), stat(s.standard_deviation));
    }
    for (name, s) in [("standardized_predicted", &rs.standardized_predicted), ("standardized_residual", &rs.standardized_residual)] {
        put(format!("residuals.{name}.minimum"), stat(s.minimum));
        put(format!("residuals.{name}.maximum"), stat(s.maximum));
    }
    put(
        "note.essentially_perfect_fit".into(),
        Got::Number(if r.notes.contains(&LinearRegressionCode::EssentiallyPerfectFit) { 1.0 } else { 0.0 }),
    );
    let columns: Vec<Alias> = r.columns.iter().map(|c| c.column).collect();
    for (j, col) in columns.iter().enumerate() {
        for reason in ["missing_value", "missing_code"] {
            let n = r
                .exclusions
                .iter()
                .filter(|e| e.code.wire() == format!("stats.linear_regression.{reason}"))
                .filter(|e| e.params.get(&ParamKey::Column) == Some(&ParamValue::Int(i64::from(col.0 .0))))
                .map(|e| e.count)
                .sum::<u64>();
            put(format!("excluded.{j}.{reason}"), Got::Number(n as f64));
        }
    }
    m
}

fn param(e: &gaply_core::contracts::Diagnostic<LinearRegressionCode>, key: ParamKey) -> Option<i64> {
    match e.params.get(&key) {
        Some(ParamValue::Int(v)) => Some(*v),
        _ => None,
    }
}

#[derive(Default)]
struct Worst(f64, String);

#[test]
fn every_number_of_every_case_is_within_its_gate_of_r() {
    let want_all = expected();
    let cases = cases();
    let mut worst: BTreeMap<String, Worst> = BTreeMap::new();
    let mut over = Vec::new();
    let mut compared = 0usize;
    let (mut refusals, mut results, mut left_out) = (0, 0, Vec::new());
    for case in &cases {
        let want = want_all.get(&case.name).unwrap_or_else(|| panic!("{}: no expected values", case.name));
        let d = data(case);
        let params = LinearRegressionParams {
            outcome: alias(&d, &case.outcome),
            predictors: case.predictors.iter().map(|h| alias(&d, h)).collect(),
            missing_codes: case.missing_codes.clone(),
        };
        if let Some(code) = want.get("refused") {
            refusals += 1;
            let e = compute(&d, &params)
                .err()
                .unwrap_or_else(|| panic!("{}: R refuses with {code}, gaply computed", case.name));
            assert_eq!(&e.code.wire().to_string(), code, "{}: the first refusal", case.name);
            let report = validate(&d, &spec(OperationId(1), &d, params.clone()));
            // The run's own two refusals come after validation, which passes.
            let run_only = matches!(e.code, LinearRegressionCode::PredictorsCollinear | LinearRegressionCode::FitOutOfRange);
            assert_eq!(
                report.errors.first().map(|x| x.code.wire().to_string()),
                if run_only { None } else { Some(code.clone()) },
                "{}: validate's first error",
                case.name
            );
            for (key, pk) in [("refused.index", ParamKey::Index), ("refused.count", ParamKey::Count), ("refused.limit", ParamKey::Limit)] {
                if let Some(v) = want.get(key) {
                    assert_eq!(param(&e, pk), Some(v.parse::<i64>().unwrap()), "{} {key}", case.name);
                }
            }
            match want.get("refused.column") {
                Some(header) => assert_eq!(param(&e, ParamKey::Column), Some(i64::from(alias(&d, header).0 .0)), "{} column", case.name),
                None => assert_eq!(param(&e, ParamKey::Column), None, "{}: a column R does not name", case.name),
            }
            continue;
        }
        results += 1;
        let r = compute(&d, &params).unwrap_or_else(|e| panic!("{}: refused {e:?}", case.name));
        let got = keys(&r);
        let ill = ILL_CONDITIONED.contains(&case.name.as_str());
        let residual_sd: f64 = want["residuals.residual.standard_deviation"].parse().unwrap();
        let scale_sd = if residual_sd == 0.0 { want["column.0.sd"].parse().unwrap() } else { residual_sd };
        let perfect = case.name == "essentially_perfect";
        // The case's largest |coefficient|: the scale of a coefficient that is
        // rounding noise about 0, as the exact tier's is.
        let largest_b = want
            .iter()
            .filter(|(k, _)| k.ends_with(".b"))
            .map(|(_, v)| v.parse::<f64>().unwrap().abs())
            .fold(0.0, f64::max);
        for (key, w) in want {
            let g = got.get(key).unwrap_or_else(|| panic!("{}: the result has no {key}", case.name));
            // The not-computable codes are compared in every case, essentially_perfect
            // included (decision 671; the review of 9A-1, finding 2).
            if let Got::Nc(c) = g {
                assert_eq!(&format!("nc {c}"), w, "{} {key}: not computable", case.name);
                continue;
            }
            if case.name == "essentially_perfect" && !perfect_fit_compared(key) {
                // A number R gives as a code would fail here, never silently pass.
                assert!(!w.starts_with("nc "), "{} {key}: R gives {w}, gaply a number", case.name);
                left_out.push(key.clone());
                continue;
            }
            match g {
                Got::Nc(c) => assert_eq!(&format!("nc {c}"), w, "{} {key}: not computable", case.name),
                Got::Number(v) => {
                    let wv: f64 = w.parse().unwrap_or_else(|_| panic!("{} {key}: gaply {v} where R gives {w}", case.name));
                    compared += 1;
                    if exact(key) {
                        assert_eq!(*v, wv, "{} {key}", case.name);
                        continue;
                    }
                    let fam = family(key);
                    let err = if *v == wv {
                        0.0
                    } else {
                        match (fam, residual_scale(key)) {
                            ("p", _) => if wv == 0.0 { v.abs() } else { (v - wv).abs() / wv.abs() },
                            (_, Some(Scale::Own)) => (v - wv).abs() / wv.abs(),
                            (_, Some(Scale::ResidualSd)) => (v - wv).abs() / scale_sd,
                            _ if perfect && key.ends_with(".b") => (v - wv).abs() / largest_b,
                            _ => (v - wv).abs() / wv.abs().max(1e-12),
                        }
                    };
                    let tier = if perfect {
                        "perfect".to_string()
                    } else if ill {
                        format!("{fam}_ill")
                    } else {
                        fam.to_string()
                    };
                    let gate = GATES.iter().find(|(f, _)| *f == tier).unwrap().1;
                    if err > gate {
                        over.push(format!("{} {key}: gaply {v:e} R {wv:e} error {err:e} gate {gate:e}", case.name));
                    }
                    let w = worst.entry(tier).or_default();
                    if err >= w.0 {
                        *w = Worst(err, format!("{} {key}", case.name));
                    }
                }
            }
        }
        // And the result carries no number R does not have.
        for key in got.keys() {
            assert!(want.contains_key(key), "{}: {key} has no R value", case.name);
        }
    }
    for (fam, gate) in GATES {
        let w = worst.get(*fam).map(|w| (w.0, w.1.as_str())).unwrap_or((0.0, "none"));
        println!("{fam:<10} largest error {:.3e} ({})  gate {gate:e}", w.0, w.1);
    }
    println!("{results} results, {refusals} refusals, {compared} numbers compared, {} keys of essentially_perfect left out: {left_out:?}", left_out.len());
    assert_eq!((results, refusals), (14, 15));
    assert_eq!(compared, 2497, "every number the generator wrote but the left-out keys");
    assert_eq!(left_out.len(), 65, "essentially_perfect's left-out keys");
    assert!(over.is_empty(), "over the gate:\n{}", over.join("\n"));
}

/// The exact tier (decision 671): each case of up to 2,000 rows solved in exact
/// rational arithmetic by `make_linear_regression_exact.R` (CRAN's gmp), rounded
/// once to the nearest double. Gaply's coefficients are compared with it, each
/// error relative to the case's largest |exact coefficient|; the gate is ten times
/// R's own largest error in the case, or 1e-14, whichever is larger. Both errors
/// are printed per case.
#[test]
fn every_coefficient_is_within_its_gate_of_the_exact_solution() {
    let text = std::fs::read_to_string(format!("{DIR}/exact.tsv")).expect("exact.tsv");
    let stamp: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(stamp.iter().any(|l| l.contains("make_linear_regression_exact.R")), "exact.tsv: not written by its script");
    assert!(stamp.iter().any(|l| l.contains(R_VERSION) && l.contains("gmp")), "exact.tsv: not written with {R_VERSION} and gmp");
    let mut by_case: BTreeMap<String, Vec<(String, f64, f64)>> = BTreeMap::new();
    for l in text.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        by_case.entry(f[0].to_string()).or_default().push((f[1].to_string(), f[2].parse().unwrap(), f[3].parse().unwrap()));
    }
    let all = cases();
    let (mut over, mut coefficients) = (Vec::new(), 0usize);
    for (name, rows) in &by_case {
        let case = all.iter().find(|c| &c.name == name).unwrap_or_else(|| panic!("{name}: not a case"));
        let d = data(case);
        let params = LinearRegressionParams {
            outcome: alias(&d, &case.outcome),
            predictors: case.predictors.iter().map(|h| alias(&d, h)).collect(),
            missing_codes: case.missing_codes.clone(),
        };
        let got = keys(&compute(&d, &params).unwrap_or_else(|e| panic!("{name}: refused {e:?}")));
        let scale = rows.iter().map(|r| r.1.abs()).fold(0.0, f64::max);
        let r_worst = rows.iter().map(|r| r.2).fold(0.0, f64::max);
        let gate = (10.0 * r_worst).max(1e-14);
        let mut gaply_worst = 0.0f64;
        for (key, exact, _) in rows {
            let Some(Got::Number(v)) = got.get(key) else { panic!("{name} {key}: not a number") };
            coefficients += 1;
            let err = (v - exact).abs() / scale;
            gaply_worst = gaply_worst.max(err);
            if err > gate {
                over.push(format!("{name} {key}: gaply {v:e} exact {exact:e} error {err:e} gate {gate:e}"));
            }
        }
        println!("{name:<22} gaply {gaply_worst:.3e}  R {r_worst:.3e}  gate {gate:.3e}");
    }
    println!("{} cases, {coefficients} coefficients against the exact solution", by_case.len());
    assert_eq!((by_case.len(), coefficients), (12, 86));
    assert!(over.is_empty(), "over the exact tier's gate:\n{}", over.join("\n"));
}
