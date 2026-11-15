//! **The AMOS tier of the structural equation models (Research OS, Phase 9C-4,
//! decisions 795 and 800).** AMOS's printed output in the IBM SPSS Amos 22
//! User's Guide: Example 6 (Wheaton's four models, two with parameters held
//! equal; pp. 103-115 and Appendix C), Example 7 (a model with a loop, Felson
//! and Bohrnstedt's 209 girls; pp. 129-135) and Example 8 fitted as a structural
//! model (decision 743's numbers). Each number the guide prints must equal
//! Gaply's value rounded to its printed decimals, a P below .001 shown as AMOS's
//! *** and HOELTER as a whole number: a difference of one in the last place
//! fails. There is no gate. Model B's 120 critical ratios for differences need
//! the estimates' covariance matrix, which the result does not carry; they are
//! a unit test in `stats_sem` (`wheaton_model_bs_critical_ratios_…`).

use gaply_core::stats_l1::dataset::{build_dataset, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_sem::{compute, SemOutcome, SemParams, Stat};

const CFA_DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_cfa/data");
const SEM_DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_sem/data");

fn data(path: &str, label: &str, provenance: &[&str], headers_want: &[&str], rows_want: usize) -> Dataset {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let header: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    for p in provenance {
        assert!(header.iter().any(|l| l.contains(p)), "{path}: its provenance names {p}");
    }
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    assert_eq!(headers, headers_want);
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    assert_eq!(rows.len(), rows_want);
    build_dataset(RawTable::from_rows(headers, rows), DataFingerprint::of_bytes(label.as_bytes(), "sem amos tier", TableRef::FirstTable)).unwrap()
}

fn spec(json: &str) -> SemParams {
    serde_json::from_str(json).unwrap()
}

fn value(s: Stat) -> f64 {
    match s {
        Stat::Value(x) => x,
        Stat::NotComputable(c) => panic!("not computable: {c:?}"),
    }
}

/// A value printed as AMOS prints it: `decimals` decimals, no leading zero.
fn printed_to(s: Stat, decimals: usize) -> String {
    let t = format!("{:.decimals$}", value(s));
    if let Some(r) = t.strip_prefix("-0.") {
        format!("-.{r}")
    } else if let Some(r) = t.strip_prefix("0.") {
        format!(".{r}")
    } else {
        t
    }
}

fn printed(s: Stat) -> String {
    printed_to(s, 3)
}

/// A P as AMOS's tables print it: *** below .001.
fn p_printed(s: Stat) -> String {
    if value(s) < 0.001 {
        "***".to_string()
    } else {
        printed(s)
    }
}

fn whole(s: Stat) -> i64 {
    let x = value(s);
    assert_eq!(x.fract(), 0.0, "HOELTER is a whole number");
    x as i64
}

fn wheaton() -> Dataset {
    data(
        &format!("{CFA_DATA}/wheaton.csv"),
        "wheaton",
        &["Example 6", "932 rows"],
        &["anomia67", "powles67", "anomia71", "powles71", "educatio", "SEI"],
        932,
    )
}

/// Wheaton's models (decision 795): alien67 (factor 1) on anomia67 and
/// powles67, alien71 (factor 2) on anomia71 and powles71, ses (factor 3) on
/// educatio and SEI; alien67 ← ses, alien71 ← alien67 and ses. B adds the
/// covariance of anomia67's and anomia71's errors; C is B with the powles
/// loadings equal and the anomia and powles error variances pairwise equal; D
/// is A with C's equalities.
fn wheaton_model(model: char) -> SemParams {
    let covariance = if model == 'B' || model == 'C' { r#"{"a":{"column":"C1"},"b":{"column":"C3"}}"# } else { "" };
    let equal = if model == 'C' || model == 'D' {
        r#"[{"loading":{"factor":1,"column":"C2"}},{"loading":{"factor":2,"column":"C4"}}],
           [{"variance":{"variable":{"column":"C1"}}},{"variance":{"variable":{"column":"C3"}}}],
           [{"variance":{"variable":{"column":"C2"}}},{"variance":{"variable":{"column":"C4"}}}]"#
    } else {
        ""
    };
    spec(&format!(
        r#"{{"indicators":[["C1","C2"],["C3","C4"],["C5","C6"]],
            "paths":[{{"to":{{"factor":1}},"from":{{"factor":3}}}},{{"to":{{"factor":2}},"from":{{"factor":1}}}},{{"to":{{"factor":2}},"from":{{"factor":3}}}}],
            "covariances":[{covariance}],"equal":[{equal}],"missingCodes":[]}}"#
    ))
}

/// Example 6 (pp. 104, 108, 115 and Appendix C; decisions 763 and 795): each
/// model's CMIN, DF, P, NPAR, CMIN/DF, RMR, GFI, AGFI, PGFI and HOELTER .05 and
/// .01, every model solved by the first attempt.
#[test]
fn wheatons_four_models_are_amoss_printed_output() {
    let d = wheaton();
    let printed_rows: [(char, &str, u64, &str, u64, &str, &str, &str, &str, &str, i64, i64); 4] = [
        ('A', "71.544", 6, ".000", 15, "11.924", ".284", ".975", ".913", ".279", 164, 219),
        ('B', "6.383", 5, ".271", 16, "1.277", ".757", ".998", ".990", ".238", 1615, 2201),
        ('C', "7.501", 8, ".484", 13, ".938", ".749", ".997", ".993", ".380", 1925, 2494),
        ('D', "73.077", 9, ".000", 12, "8.120", ".263", ".975", ".941", ".418", 216, 277),
    ];
    for (model, cmin, df, p, npar, cmin_df, rmr, gfi, agfi, pgfi, h05, h01) in printed_rows {
        let r = compute(&d, &wheaton_model(model)).unwrap_or_else(|e| panic!("Model {model}: {e:?}"));
        let f = &r.fit.model;
        assert_eq!(r.attempt, 1, "Model {model}: the first attempt");
        assert_eq!(printed(f.cmin), cmin, "Model {model} CMIN");
        assert_eq!(f.df, df, "Model {model} DF");
        assert_eq!(printed(f.p), p, "Model {model} P");
        assert_eq!(f.npar, npar, "Model {model} NPAR");
        assert_eq!(printed(f.cmin_df), cmin_df, "Model {model} CMIN/DF");
        assert_eq!(printed(f.rmr), rmr, "Model {model} RMR");
        assert_eq!(printed(f.gfi), gfi, "Model {model} GFI");
        assert_eq!(printed(f.agfi), agfi, "Model {model} AGFI");
        assert_eq!(printed(f.pgfi), pgfi, "Model {model} PGFI");
        assert_eq!(whole(f.hoelter05), h05, "Model {model} HOELTER .05");
        assert_eq!(whole(f.hoelter01), h01, "Model {model} HOELTER .01");
    }
}

/// Example 7 (pp. 129-135; decision 795): academic (C1) ← GPA (C3) and attract
/// (C2); attract ← height (C4), weight (C5), rating (C6) and academic; the
/// errors of academic and attract correlated; the six covariances among GPA,
/// height, weight and rating.
fn example_7() -> (Dataset, SemParams) {
    let d = data(
        &format!("{SEM_DATA}/felson.csv"),
        "felson",
        &["Example 7", "felson1979", "209 rows"],
        &["academic", "attract", "GPA", "height", "weight", "rating"],
        209,
    );
    let s = spec(
        r#"{"indicators":[],
            "paths":[{"to":{"column":"C1"},"from":{"column":"C2"}},{"to":{"column":"C1"},"from":{"column":"C3"}},
                     {"to":{"column":"C2"},"from":{"column":"C1"}},{"to":{"column":"C2"},"from":{"column":"C4"}},
                     {"to":{"column":"C2"},"from":{"column":"C5"}},{"to":{"column":"C2"},"from":{"column":"C6"}}],
            "covariances":[{"a":{"column":"C1"},"b":{"column":"C2"}},{"a":{"column":"C3"},"b":{"column":"C4"}},
                           {"a":{"column":"C3"},"b":{"column":"C5"}},{"a":{"column":"C3"},"b":{"column":"C6"}},
                           {"a":{"column":"C4"},"b":{"column":"C5"}},{"a":{"column":"C4"},"b":{"column":"C6"}},
                           {"a":{"column":"C5"},"b":{"column":"C6"}}],
            "equal":[],"missingCodes":[]}"#,
    );
    (d, s)
}

/// Every number Example 7 prints: CMIN, DF and P; the regression weights, the
/// covariances and the variances with their standard errors, C.R.s and Ps; the
/// standardized regression weights; the correlations; the squared multiple
/// correlations. Solved by the first attempt.
#[test]
fn example_7_is_amoss_printed_output_to_its_last_decimal() {
    let (d, s) = example_7();
    let r: SemOutcome = compute(&d, &s).expect("Example 7 fits");
    assert_eq!(r.attempt, 1);
    assert_eq!(printed(r.fit.model.cmin), "2.761", "CMIN");
    assert_eq!(r.fit.model.df, 2, "DF");
    assert_eq!(printed(r.fit.model.p), ".251", "P");
    // The weights in AMOS's order, by this result's index: academic ← GPA (1),
    // attract ← height (3), weight (4), rating (5), academic (2), academic ←
    // attract (0).
    let w = &r.weights;
    let order = [1usize, 3, 4, 5, 2, 0];
    let printed_weights = [
        (".023", ".004", "6.241", "***"),
        (".000", ".010", ".050", ".960"),
        ("-.002", ".001", "-1.321", ".186"),
        (".176", ".027", "6.444", "***"),
        ("1.607", ".349", "4.599", "***"),
        ("-.002", ".051", "-.039", ".969"),
    ];
    for (i, want) in order.iter().zip(printed_weights) {
        let x = &w[*i];
        assert_eq!((printed(x.estimate), printed(x.se), printed(x.cr), p_printed(x.p)), (want.0.into(), want.1.into(), want.2.into(), want.3.into()), "weight {i}");
    }
    let standardized: Vec<String> = order.iter().map(|i| printed(w[*i].standardized)).collect();
    assert_eq!(standardized, [".492", ".003", "-.078", ".363", ".525", "-.006"], "the standardized regression weights");
    // The covariances in AMOS's order: GPA–rating (3), height–rating (5),
    // GPA–weight (2), GPA–height (1), height–weight (4), weight–rating (6), the
    // two errors (0).
    let c = &r.covariances;
    let order = [3usize, 5, 2, 1, 4, 6, 0];
    let printed_covariances = [
        (".526", ".246", "2.139", ".032"),
        ("-.468", ".205", "-2.279", ".023"),
        ("-6.710", "4.676", "-1.435", ".151"),
        ("1.819", ".712", "2.555", ".011"),
        ("19.024", "4.098", "4.643", "***"),
        ("-5.243", "1.395", "-3.759", "***"),
        ("-.004", ".010", "-.382", ".702"),
    ];
    for (i, want) in order.iter().zip(printed_covariances) {
        let x = &c[*i];
        assert_eq!((printed(x.estimate), printed(x.se), printed(x.cr), p_printed(x.p)), (want.0.into(), want.1.into(), want.2.into(), want.3.into()), "covariance {i}");
    }
    let correlations: Vec<String> = order.iter().map(|i| printed(c[*i].correlation)).collect();
    assert_eq!(correlations, [".150", "-.160", "-.100", ".180", ".340", "-.270", "-.076"], "the correlations");
    // The variances in AMOS's order: GPA, height, weight, rating (C3 to C6),
    // then academic's and attract's errors (C1, C2): this result's 2 to 5, 0, 1.
    let v = &r.variances;
    let printed_variances = [
        (2usize, "12.122", "1.189", "10.198"),
        (3, "8.428", ".826", "10.198"),
        (4, "371.476", "36.426", "10.198"),
        (5, "1.015", ".100", "10.198"),
        (0, ".019", ".003", "5.747"),
        (1, ".143", ".014", "9.974"),
    ];
    for (i, est, se, cr) in printed_variances {
        let x = &v[i];
        assert_eq!((printed(x.estimate), printed(x.se), printed(x.cr), p_printed(x.p)), (est.into(), se.into(), cr.into(), "***".into()), "variance {i}");
        assert_eq!(x.error, i < 2, "variance {i}: an error term exactly for academic and attract");
    }
    // The squared multiple correlations: attract .402, academic .236.
    assert_eq!(r.smc.len(), 2);
    assert_eq!((printed(r.smc[1].value), printed(r.smc[0].value)), (".402".into(), ".236".into()), "the squared multiple correlations");
}

/// Example 8 fitted as a structural model (decision 743's printed numbers):
/// spatial (factor 1) on visperc, cubes and lozenges, verbal (factor 2) on
/// paragrap, sentence and wordmean, their covariance free.
#[test]
fn example_8_as_a_structural_model_is_amoss_printed_output() {
    let d = data(
        &format!("{CFA_DATA}/example8.csv"),
        "example 8",
        &["Example 8", "HolzingerSwineford1939", "Grant-White"],
        &["visperc", "cubes", "lozenges", "paragrap", "sentence", "wordmean"],
        73,
    );
    let s = spec(r#"{"indicators":[["C1","C2","C3"],["C4","C5","C6"]],"paths":[],"covariances":[{"a":{"factor":1},"b":{"factor":2}}],"equal":[],"missingCodes":[]}"#);
    let r = compute(&d, &s).expect("Example 8 fits");
    assert_eq!(r.attempt, 1);
    let w = &r.weights;
    let v = &r.variances;
    // AMOS's order: cubes, lozenges, sentence, wordmean; the covariance; the
    // variances of spatial and verbal; the six errors.
    let cells = |pick: fn(&gaply_core::stats_sem::SemWeight) -> Stat, cov: Stat, vpick: fn(&gaply_core::stats_sem::SemVariance) -> Stat| -> Vec<String> {
        [pick(&w[1]), pick(&w[2]), pick(&w[4]), pick(&w[5]), cov].into_iter().chain(v.iter().map(vpick)).map(printed).collect()
    };
    assert_eq!(cells(|x| x.estimate, r.covariances[0].estimate, |x| x.estimate), [".610", "1.198", "1.334", "2.234", "7.315", "23.302", "9.682", "23.873", "11.602", "28.275", "2.834", "7.967", "19.925"], "the estimates");
    assert_eq!(cells(|x| x.se, r.covariances[0].se, |x| x.se), [".143", ".272", ".160", ".263", "2.571", "8.123", "2.159", "5.986", "2.584", "7.892", ".868", "1.869", "4.951"], "the standard errors");
    assert_eq!(cells(|x| x.cr, r.covariances[0].cr, |x| x.cr), ["4.250", "4.405", "8.322", "8.482", "2.846", "2.868", "4.485", "3.988", "4.490", "3.583", "3.263", "4.263", "4.024"], "the critical ratios");
    let standardized: Vec<String> = w.iter().map(|x| printed(x.standardized)).collect();
    assert_eq!(standardized, [".703", ".654", ".736", ".880", ".827", ".841"], "the standardized regression weights");
    assert_eq!(printed(r.covariances[0].correlation), ".487", "the correlation");
    let smc: Vec<String> = r.smc.iter().map(|x| printed(x.value)).collect();
    assert_eq!(smc, [".494", ".428", ".542", ".774", ".684", ".708"], "the squared multiple correlations");
    assert_eq!(printed(r.fit.model.cmin), "7.853", "CMIN");
    assert_eq!(r.fit.model.df, 8, "DF");
    assert_eq!(printed(r.fit.model.p), ".448", "P");
    assert_eq!(r.fit.model.npar, 13, "NPAR");
}
