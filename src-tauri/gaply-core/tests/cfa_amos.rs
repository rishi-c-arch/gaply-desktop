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
    assert_eq!(printed(r.fit.model.cmin), "7.853", "CMIN");
    assert_eq!(r.fit.model.df, 8, "DF");
    assert_eq!(printed(r.fit.model.p), ".448", "P");
    assert_eq!(r.fit.model.npar, 13, "NPAR");
}

/// Research OS, Phase 9C-2 (decision 763): the independence and saturated rows
/// AMOS printed for the Wheaton data (the IBM SPSS Amos User's Guide, Example 6
/// and Appendix C), which depend only on S and N, so any CFA on those six
/// columns gives them. data/wheaton.csv is written by make_cfa_fixtures.R from
/// the correlations and standard deviations decision 763 quotes, its S with
/// divisor N being S_u (N − 1) / N (reproduced, not read from Wheaton.sav). The
/// model: (anomia67, powles67, anomia71, powles71) and (educatio, SEI),
/// correlated. Each cell at its printed decimals, HOELTER as a whole number;
/// the saturated row's P, CMIN/DF, AGFI and PGFI blank in AMOS, df_zero here.
#[test]
fn the_wheaton_independence_and_saturated_rows_are_amos_s_printed_output() {
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_cfa/data/wheaton.csv");
    let text = std::fs::read_to_string(file).expect("wheaton.csv");
    let provenance: Vec<&str> = text.lines().take_while(|l| l.starts_with('#')).collect();
    assert!(provenance.iter().any(|l| l.contains("Example 6") && l.contains("932 rows")));
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().unwrap().split(',').map(str::to_string).collect();
    assert_eq!(headers, ["anomia67", "powles67", "anomia71", "powles71", "educatio", "SEI"]);
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    assert_eq!(rows.len(), 932);
    let d = build_dataset(RawTable::from_rows(headers, rows), DataFingerprint::of_bytes(b"wheaton", "amos tier", TableRef::FirstTable)).unwrap();
    let a = |n: u32| Alias(ColumnAlias(n));
    let params = CfaParams { indicators: vec![vec![a(1), a(2), a(3), a(4)], vec![a(5), a(6)]], correlated_factors: true, missing_codes: vec![] };
    let r = compute(&d, &params).expect("the Wheaton model fits");
    let whole = |s: Stat| {
        let Stat::Value(x) = s else { panic!("not computable: {s:?}") };
        assert_eq!(x.fract(), 0.0, "HOELTER is a whole number");
        x as i64
    };

    let b = &r.fit.independence;
    assert_eq!(b.npar, 6, "independence NPAR");
    assert_eq!(printed(b.cmin), "2131.790", "independence CMIN");
    assert_eq!(b.df, 15, "independence DF");
    assert_eq!(printed(b.p), ".000", "independence P");
    assert_eq!(printed(b.cmin_df), "142.119", "independence CMIN/DF");
    assert_eq!(printed(b.rmr), "12.342", "independence RMR");
    assert_eq!(printed(b.gfi), ".494", "independence GFI");
    assert_eq!(printed(b.agfi), ".292", "independence AGFI");
    assert_eq!(printed(b.pgfi), ".353", "independence PGFI");
    assert_eq!(whole(b.hoelter05), 11, "independence HOELTER .05");
    assert_eq!(whole(b.hoelter01), 14, "independence HOELTER .01");

    let s = &r.fit.saturated;
    assert_eq!(s.npar, 21, "saturated NPAR");
    assert_eq!(printed(s.cmin), ".000", "saturated CMIN");
    assert_eq!(s.df, 0, "saturated DF");
    assert_eq!(printed(s.rmr), ".000", "saturated RMR");
    assert_eq!(printed(s.gfi), "1.000", "saturated GFI");
    for (name, x) in [("P", s.p), ("CMIN/DF", s.cmin_df), ("AGFI", s.agfi), ("PGFI", s.pgfi)] {
        assert_eq!(x, Stat::NotComputable(gaply_core::stats_cfa::CfaCode::DfZero), "saturated {name}: blank in AMOS");
    }
}
