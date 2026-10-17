//! How closely Level 1's computation matches R, measured over
//! `tests/fixtures/stats_l1/golden.csv` (written with `/usr/local/bin/Rscript`
//! by `make_l1_fixtures.R`, which also writes the synthetic `data_*.csv` files
//! and says what R call produced each row).
//!
//! Every row goes through the path a user's claim takes: the data file's
//! bytes become a `RawTable` and a `Dataset`, a claim is added, a binding is
//! proposed and CONFIRMED, and `compute` runs on a dataset built again from
//! the same bytes with the token the confirmation issued. Nothing here calls
//! `stats_verify` directly.
//!
//! A GATE, per method: the largest relative difference from R over the
//! method's rows, on the statistic, each df and p, must be within the
//! registry's `relative_gate`; `n` must be equal. It prints the measured
//! maxima per method:
//!
//! ```text
//! cargo test -p gaply_core --test l1_r_golden -- --nocapture
//! ```

use std::collections::BTreeMap;

use gaply_core::extract::SectionKind;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, ProposalSource};
use gaply_core::stats_l1::catalogue::{L1MethodId, MethodCatalogue, RoleName, VariantChoice};
use gaply_core::stats_l1::claim::{ClaimLocation, UserClaim, UserP};
use gaply_core::stats_l1::compute::{compute, ComputedResult, ComputedVariant, L1Registry, OlsTerm};
use gaply_core::stats_l1::binding::OlsTarget;
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};
use gaply_core::stats_l1::run_state::L1RunState;

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_l1");

struct Golden {
    case: String,
    data: String,
    method: String,
    roles: String,
    variant: String,
    statistic: f64,
    df1: f64,
    df2: Option<f64>,
    p: f64,
    n: usize,
}

fn num(s: &str) -> Option<f64> {
    (s != "NA").then(|| s.parse::<f64>().unwrap_or_else(|e| panic!("bad number {s:?}: {e}")))
}

fn load() -> (Vec<String>, Vec<Golden>) {
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
            assert_eq!(line, "case,data,method,roles,variant,r_call,statistic,df1,df2,p,n", "column header");
            columns_seen = true;
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        assert_eq!(f.len(), 11, "row {line:?}");
        rows.push(Golden {
            case: f[0].into(),
            data: f[1].into(),
            method: f[2].into(),
            roles: f[3].into(),
            variant: f[4].into(),
            statistic: num(f[6]).expect("statistic"),
            df1: num(f[7]).expect("df1"),
            df2: num(f[8]),
            p: num(f[9]).expect("p"),
            n: f[10].parse().expect("n"),
        });
    }
    (header, rows)
}

/// The data file: bytes, headers and rows, '#' header lines skipped. The
/// fixtures hold no quoted fields.
fn data(name: &str) -> (Vec<u8>, RawTable) {
    let bytes = std::fs::read(format!("{DIR}/{name}")).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let headers: Vec<String> = lines.next().expect("header").split(',').map(str::to_string).collect();
    let rows: Vec<Vec<String>> = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
    (bytes, RawTable::from_rows(headers, rows))
}

fn dataset(bytes: &[u8], raw: RawTable) -> Dataset {
    build_dataset(raw, DataFingerprint::of_bytes(bytes, "golden csv", TableRef::FirstTable)).expect("dataset")
}

fn alias(headers: &[String], name: &str) -> ColumnAlias {
    let i = headers.iter().position(|h| h == name).unwrap_or_else(|| panic!("no column {name}"));
    ColumnAlias(i as u32 + 1)
}

fn role_name(s: &str) -> RoleName {
    match s {
        "outcome" => RoleName::Outcome,
        "group" => RoleName::Group,
        "first" => RoleName::First,
        "second" => RoleName::Second,
        "x" => RoleName::X,
        "y" => RoleName::Y,
        "row" => RoleName::RowVariable,
        "column" => RoleName::ColumnVariable,
        "predictors" => RoleName::Predictors,
        other => panic!("unknown role {other}"),
    }
}

fn variant(s: &str, headers: &[String]) -> ComputedVariant {
    match s {
        "student" => ComputedVariant::StudentPooled,
        "welch" => ComputedVariant::Welch,
        "paired" => ComputedVariant::Paired,
        "oneway" => ComputedVariant::OneWay,
        "pearson" => ComputedVariant::Pearson,
        "spearman_t" => ComputedVariant::SpearmanTApproximation,
        "chi_uncorrected" => ComputedVariant::ChiSquareUncorrected,
        "chi_yates_r" => ComputedVariant::ChiSquareYatesR,
        "ols_f" => ComputedVariant::OlsOverallF,
        "ols_coef:(Intercept)" => ComputedVariant::OlsCoefficient(OlsTerm::Intercept),
        other => match other.strip_prefix("ols_coef:") {
            Some(column) => ComputedVariant::OlsCoefficient(OlsTerm::Predictor(alias(headers, column))),
            None => panic!("unknown variant {other}"),
        },
    }
}

/// Every result for one golden row's binding, through claim, confirm, token
/// and compute.
fn run(g: &Golden) -> (Vec<String>, Vec<ComputedResult>) {
    let (bytes, raw) = data(&g.data);
    let headers = raw.headers.clone();
    let method = L1MethodId::from_id(&g.method).expect("method id");
    let mut draft = BindingDraft::new(method, VariantChoice::not_stated(MethodCatalogue::entry(method).variant_kind));
    for pair in g.roles.split(';') {
        let (role, cols) = pair.split_once('=').expect("role=columns");
        let cols: Vec<ColumnAlias> = cols.split('|').map(|c| alias(&headers, c)).collect();
        draft = draft.role(role_name(role), &cols);
    }
    if method == L1MethodId::Ols {
        draft = draft.target(OlsTarget::OverallF);
    }

    let mut state = L1RunState::new();
    state.load_dataset(dataset(&bytes, raw.clone()));
    let claim = state
        .add_claim(&UserClaim {
            family: Family::T,
            statistic: "1.00",
            df: vec!["10"],
            n: None,
            p: UserP::Reported { raw: ".05", relation: Relation::Eq },
            location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 },
            sentence: None,
        })
        .expect("claim");
    let proposal = state.add_proposal(claim, draft, ProposalSource::Manual { edited_from: None }).expect("proposal");
    let token = state.confirm(proposal).unwrap_or_else(|e| panic!("{}: confirm {e:?}", g.case)).1;
    // The data again, from the same bytes.
    let reread = dataset(&bytes, raw);
    let c = compute(&state, &token, claim, &reread).unwrap_or_else(|e| panic!("{}: compute {e:?}", g.case));
    (headers, c.results)
}

fn rel(engine: f64, r: f64) -> f64 {
    if r == 0.0 {
        engine.abs()
    } else {
        ((engine - r) / r).abs()
    }
}

#[test]
fn level_1_matches_r_on_every_golden_row_within_each_methods_gate() {
    let (header, rows) = load();
    assert!(header.iter().any(|h| h.contains("/usr/local/bin/Rscript make_l1_fixtures.R")), "{header:?}");
    assert!(header.iter().any(|h| h.starts_with("R version")), "{header:?}");
    // 43 rows on 14 Oct 2026. Fewer and the gate is reading something else.
    assert!(rows.len() >= 43, "only {} golden rows", rows.len());

    // method -> (rows, worst relative difference, the case it occurs on)
    let mut worst: BTreeMap<String, (usize, f64, String)> = BTreeMap::new();
    let mut variants_seen: BTreeMap<String, usize> = BTreeMap::new();
    for g in &rows {
        let (headers, results) = run(g);
        let want = variant(&g.variant, &headers);
        let got = results.iter().find(|r| r.variant == want).unwrap_or_else(|| panic!("{}: no {want:?} in {results:?}", g.case));
        assert_eq!(got.n, g.n, "{}: n", g.case);
        let mut diffs = vec![rel(got.value, g.statistic), rel(got.df1, g.df1), rel(got.p, g.p)];
        match (got.df2, g.df2) {
            (Some(a), Some(b)) => diffs.push(rel(a, b)),
            (None, None) => {}
            other => panic!("{}: df2 {other:?}", g.case),
        }
        let d = diffs.into_iter().fold(0.0, f64::max);
        let e = worst.entry(g.method.clone()).or_insert((0, 0.0, String::new()));
        e.0 += 1;
        if d >= e.1 {
            e.1 = d;
            e.2 = g.case.clone();
        }
        *variants_seen.entry(g.variant.split(':').next().unwrap().to_string()).or_default() += 1;
    }

    println!("method                     rows   worst relative difference from R   (case)        gate");
    for (method, (n, d, case)) in &worst {
        let gate = L1Registry::by_id(method).expect("registered").relative_gate;
        println!("{method:26} {n:>4}   {d:>12.3e}                       ({case})   {gate:e}");
    }
    for (method, (_, d, case)) in &worst {
        let gate = L1Registry::by_id(method).expect("registered").relative_gate;
        assert!(*d <= gate, "{method}: {d:e} on {case} exceeds the gate {gate:e}");
    }
    // Every registered method and every variant has a golden row.
    for m in L1Registry::METHODS {
        assert!(worst.contains_key(m.method.id()), "{} has no golden row", m.method.id());
    }
    for v in ["student", "welch", "paired", "oneway", "pearson", "spearman_t", "chi_uncorrected", "chi_yates_r", "ols_f", "ols_coef"] {
        assert!(variants_seen.contains_key(v), "no golden row for variant {v}");
    }
}

/// A 2×2 table gets the uncorrected statistic and R's continuity correction,
/// nothing else, including the table whose |O - E| is under 0.5 (where a
/// plain 0.5 with no floor would have differed); a 3×3 gets the uncorrected
/// statistic only.
#[test]
fn a_2x2_gets_exactly_the_uncorrected_and_r_yates_variants() {
    let (_, rows) = load();
    for case in ["chi_2x2_uncorrected", "chi_2x2_small_diff_uncorrected", "chi_3x3_uncorrected"] {
        let g = rows.iter().find(|g| g.case == case).expect(case);
        let (_, results) = run(g);
        let variants: Vec<ComputedVariant> = results.iter().map(|r| r.variant).collect();
        let want = if case == "chi_3x3_uncorrected" {
            vec![ComputedVariant::ChiSquareUncorrected]
        } else {
            vec![ComputedVariant::ChiSquareUncorrected, ComputedVariant::ChiSquareYatesR]
        };
        assert_eq!(variants, want, "{case}");
    }
}
