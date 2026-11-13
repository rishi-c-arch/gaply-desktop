//! **The AMOS tier (Research OS, Phase 9C-1, decisions 743 and 746).** AMOS's
//! Example 8 (the IBM SPSS Amos 22 User's Guide): the 73 Grant-White girls of
//! lavaan's HolzingerSwineford1939 in AMOS's units (visperc = 6 x1, cubes = 4 x2,
//! lozenges = 8 x3, paragrap = 3 x4, sentence = 4 x5, wordmean = 7 x6), spatial
//! measured by the first three and verbal by the last three, correlated; the
//! data file is written by make_cfa_fixtures.R with that provenance. Each
//! number the guide prints must equal Gaply's value rounded to its printed
//! decimals: a difference of one in the last place fails. There is no gate.

use gaply_core::stats_cfa::{compute, CfaParams, Stat};
use gaply_core::stats_desc::Alias;
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, RawTable, TableRef};

const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_cfa/data/example8.csv");

/// A value printed as AMOS prints it: three decimals, no leading zero.
fn printed(s: Stat) -> String {
    let Stat::Value(x) = s else { panic!("not computable: {s:?}") };
    let t = format!("{x:.3}");
    t.strip_prefix("0.").map(|r| format!(".{r}")).unwrap_or(t)
}

#[test]
fn example_8_is_amos_s_printed_output_to_its_last_decimal() {
    let text = std::fs::read_to_string(FILE).expect("example8.csv");
    let provenance: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(provenance.iter().any(|l| l.contains("Example 8") && l.contains("HolzingerSwineford1939") && l.contains("Grant-White")));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    assert_eq!(headers, ["visperc", "cubes", "lozenges", "paragrap", "sentence", "wordmean"]);
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    assert_eq!(rows.len(), 73);
    let d = build_dataset(RawTable::from_rows(headers, rows), DataFingerprint::of_bytes(b"example 8", "amos tier", TableRef::FirstTable)).unwrap();
    let a = |n: u32| Alias(ColumnAlias(n));
    let params = CfaParams { indicators: vec![vec![a(1), a(2), a(3)], vec![a(4), a(5), a(6)]], correlated_factors: true, missing_codes: vec![] };
    let r = compute(&d, &params).expect("Example 8 fits");

    // AMOS's order: cubes, lozenges, sentence, wordmean on their factors; the
    // covariance of spatial and verbal; the variances of spatial and verbal;
    // the six errors.
    let l = &r.loadings;
    let estimates = [l[1].estimate, l[2].estimate, l[4].estimate, l[5].estimate, r.factor_covariances[0].estimate, r.factor_variances[0].estimate, r.factor_variances[1].estimate]
        .into_iter()
        .chain(r.error_variances.iter().map(|e| e.estimate));
    let ses = [l[1].se, l[2].se, l[4].se, l[5].se, r.factor_covariances[0].se, r.factor_variances[0].se, r.factor_variances[1].se]
        .into_iter()
        .chain(r.error_variances.iter().map(|e| e.se));
    let crs = [l[1].cr, l[2].cr, l[4].cr, l[5].cr, r.factor_covariances[0].cr, r.factor_variances[0].cr, r.factor_variances[1].cr]
        .into_iter()
        .chain(r.error_variances.iter().map(|e| e.cr));
    let got: Vec<String> = estimates.map(printed).collect();
    assert_eq!(got, [".610", "1.198", "1.334", "2.234", "7.315", "23.302", "9.682", "23.873", "11.602", "28.275", "2.834", "7.967", "19.925"], "the estimates");
    let got: Vec<String> = ses.map(printed).collect();
    assert_eq!(got, [".143", ".272", ".160", ".263", "2.571", "8.123", "2.159", "5.986", "2.584", "7.892", ".868", "1.869", "4.951"], "the standard errors");
    let got: Vec<String> = crs.map(printed).collect();
    assert_eq!(got, ["4.250", "4.405", "8.322", "8.482", "2.846", "2.868", "4.485", "3.988", "4.490", "3.583", "3.263", "4.263", "4.024"], "the critical ratios");
    let got: Vec<String> = l.iter().map(|x| printed(x.standardized)).collect();
    assert_eq!(got, [".703", ".654", ".736", ".880", ".827", ".841"], "the standardized regression weights");
    assert_eq!(printed(r.factor_covariances[0].correlation), ".487", "the correlation");
    let got: Vec<String> = r.smc.iter().map(|x| printed(x.value)).collect();
    assert_eq!(got, [".494", ".428", ".542", ".774", ".684", ".708"], "the squared multiple correlations");
    assert_eq!(printed(r.fit.cmin), "7.853", "CMIN");
    assert_eq!(r.fit.df, 8, "DF");
    assert_eq!(printed(r.fit.p), ".448", "P");
    assert_eq!(r.fit.npar, 13, "NPAR");
}
