//! **The façades give exactly the verifier's own output, over the golden
//! sets (Research OS, Phase 4A).**
//!
//! * `stats.verify`: every row of Level 1's golden set
//!   (`fixtures/stats_l1/golden.csv`, the set `l1_r_golden.rs` gates against
//!   R) goes through the path a claim takes: a claim, a proposal, a
//!   confirmation and its token. Then `stats_adapters::verify`, with the spec
//!   the app builds for the binding's method, is compared with
//!   `stats_l1::reproduce::reproduce` called directly, by equality
//!   (`ReproductionOutcome` is `PartialEq`, deliberately not `Serialize`).
//! * `stats.describe`: every case of Phase 2A's R golden set
//!   (`fixtures/stats_desc/golden.csv`) through `describe_validate` and
//!   `describe_until`, compared with `stats_desc::validate` and
//!   `stats_desc::describe` by their serde bytes.
//!
//! The loaders follow `l1_r_golden.rs` and `desc_r_golden.rs`, which stay
//! the gates against R; this file gates the façades against those functions.

use std::collections::BTreeMap;
use std::ops::ControlFlow;

use gaply_core::contracts::capability::{Build, ConfirmationId};
use gaply_core::contracts::OperationId;
use gaply_core::extract::SectionKind;
use gaply_core::stats_adapters;
use gaply_core::stats_desc::{self, Alias, ColumnRequest, DescribeParams, TreatAs};
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, OlsTarget, ProposalSource};
use gaply_core::stats_l1::catalogue::{L1MethodId, MethodCatalogue, RoleName, VariantChoice};
use gaply_core::stats_l1::claim::{ClaimLocation, UserClaim, UserP};
use gaply_core::stats_l1::dataset::{
    build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef,
};
use gaply_core::stats_l1::reproduce::reproduce;
use gaply_core::stats_l1::run_state::L1RunState;

const L1_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_l1");
const DESC_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/stats_desc");

/* ---- Level 1 ------------------------------------------------------------ */

struct Golden {
    case: String,
    data: String,
    method: String,
    roles: String,
}

fn l1_rows() -> Vec<Golden> {
    let text = std::fs::read_to_string(format!("{L1_DIR}/golden.csv")).expect("golden.csv");
    let mut lines = text.lines().filter(|l| !l.starts_with('#'));
    let header: Vec<&str> = lines.next().expect("columns").split(',').collect();
    let at = |name: &str| {
        header
            .iter()
            .position(|h| *h == name)
            .unwrap_or_else(|| panic!("no column {name}"))
    };
    let (case, data, method, roles) = (at("case"), at("data"), at("method"), at("roles"));
    lines
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            Golden {
                case: f[case].into(),
                data: f[data].into(),
                method: f[method].into(),
                roles: f[roles].into(),
            }
        })
        .collect()
}

fn l1_data(name: &str) -> (Vec<u8>, RawTable) {
    let bytes = std::fs::read(format!("{L1_DIR}/{name}")).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
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
    (bytes, RawTable::from_rows(headers, rows))
}

fn l1_dataset(bytes: &[u8], raw: RawTable) -> Dataset {
    build_dataset(
        raw,
        DataFingerprint::of_bytes(bytes, "golden csv", TableRef::FirstTable),
    )
    .expect("dataset")
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
        other => panic!("role {other}"),
    }
}

#[test]
fn stats_verify_through_its_facade_equals_reproduce_on_every_golden_row() {
    let rows = l1_rows();
    assert!(
        rows.len() >= 30,
        "{} golden rows — the file is short",
        rows.len()
    );
    let mut by_method: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut computed = 0;
    for g in &rows {
        let (bytes, raw) = l1_data(&g.data);
        let headers = raw.headers.clone();
        let method = L1MethodId::from_id(&g.method).expect("method id");
        let mut draft = BindingDraft::new(
            method,
            VariantChoice::not_stated(MethodCatalogue::entry(method).variant_kind),
        );
        for pair in g.roles.split(';') {
            let (role, cols) = pair.split_once('=').expect("role=columns");
            let cols: Vec<ColumnAlias> = cols
                .split('|')
                .map(|c| {
                    ColumnAlias(headers.iter().position(|h| h == c).expect("column") as u32 + 1)
                })
                .collect();
            draft = draft.role(role_name(role), &cols);
        }
        if method == L1MethodId::Ols {
            draft = draft.target(OlsTarget::OverallF);
        }
        // The claim's family and df shape follow the method, so the outcome
        // reaches the comparison; its numbers are placeholders.
        let (family, df) = match method {
            L1MethodId::IndependentT | L1MethodId::PairedT => (Family::T, vec!["10"]),
            L1MethodId::OneWayAnova | L1MethodId::Ols => (Family::F, vec!["2", "10"]),
            L1MethodId::Pearson | L1MethodId::Spearman => (Family::R, vec!["10"]),
            L1MethodId::ChiSquareIndependence => (Family::ChiSquare, vec!["1"]),
        };
        let mut state = L1RunState::new();
        state.load_dataset(l1_dataset(&bytes, raw.clone()));
        let claim = state
            .add_claim(&UserClaim {
                family,
                statistic: "1.00",
                df,
                n: None,
                p: UserP::Reported {
                    raw: ".05",
                    relation: Relation::Eq,
                },
                location: ClaimLocation {
                    section_index: 0,
                    section: SectionKind::Results,
                    paragraph: 0,
                    sentence: 0,
                },
                sentence: None,
            })
            .expect("claim");
        let proposal = state
            .add_proposal(claim, draft, ProposalSource::Manual { edited_from: None })
            .expect("proposal");
        let token = state
            .confirm(proposal)
            .unwrap_or_else(|e| panic!("{}: confirm {e:?}", g.case))
            .1;
        let revision = state.claim(claim).expect("claim").revision.clone();
        let pairs = vec![(revision, token.clone())];
        let read = || Ok(l1_dataset(&bytes, raw.clone()));

        let direct = reproduce(&state, &pairs, read);
        let spec = stats_adapters::verify_spec(
            method,
            vec![ConfirmationId::parse(token.as_str()).expect("hex")],
        );
        let wrapped = stats_adapters::verify(&[spec], Build::Standard, &state, &pairs, read)
            .unwrap_or_else(|e| panic!("{}: {e:?}", g.case));
        assert_eq!(wrapped, direct, "{}", g.case);
        if direct.claims[0].counts.is_some() {
            computed += 1;
        }
        *by_method.entry(method.id()).or_default() += 1;
    }
    println!(
        "golden rows compared: {} (computed {computed}), by method {by_method:?}",
        rows.len()
    );
    assert_eq!(
        by_method.len(),
        L1MethodId::ALL.len(),
        "every Level 1 method has golden rows"
    );
    assert!(
        computed * 2 > rows.len(),
        "only {computed} of {} rows reached the comparison",
        rows.len()
    );
}

/* ---- the descriptive summary -------------------------------------------- */

fn desc_cases() -> Vec<(String, String, String, String, String)> {
    let text = std::fs::read_to_string(format!("{DESC_DIR}/golden.csv")).expect("golden.csv");
    let mut seen = BTreeMap::new();
    for line in text.lines().filter(|l| !l.starts_with('#')).skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        seen.entry(f[0].to_string()).or_insert((
            f[1].to_string(),
            f[2].to_string(),
            f[3].to_string(),
            f[4].to_string(),
        ));
    }
    seen.into_iter()
        .map(|(c, (d, col, t, m))| (c, d, col, t, m))
        .collect()
}

fn desc_dataset(name: &str) -> Dataset {
    let bytes = std::fs::read(format!("{DESC_DIR}/{name}")).expect("data file");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
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
    build_dataset(
        RawTable::from_rows(headers, rows),
        DataFingerprint::of_bytes(&bytes, "golden csv", TableRef::FirstTable),
    )
    .expect("dataset")
}

#[test]
fn stats_describe_through_its_facade_equals_the_summary_on_every_r_golden_case() {
    let cases = desc_cases();
    assert!(
        cases.len() >= 19,
        "{} cases — the golden file is short",
        cases.len()
    );
    for (case, data, column, treat_as, missing) in &cases {
        let dataset = desc_dataset(data);
        let alias = dataset
            .schema
            .columns
            .iter()
            .find(|c| &c.header == column)
            .map(|c| Alias(c.alias))
            .unwrap_or_else(|| panic!("{case}: no column {column}"));
        let params = DescribeParams {
            columns: vec![ColumnRequest {
                alias,
                treat_as: if treat_as == "numeric" {
                    TreatAs::Numeric
                } else {
                    TreatAs::Categorical
                },
            }],
            missing_codes: missing
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().expect("code"))
                .collect(),
        };
        let spec = stats_desc::spec(OperationId(1), &dataset, params.clone());
        let wrapped_report = stats_adapters::describe_validate(&spec, Build::Standard, &dataset);
        let direct_report = stats_desc::validate(&dataset, &spec);
        assert_eq!(
            bytes(&wrapped_report),
            bytes(&direct_report),
            "{case}: validation"
        );
        let wrapped = stats_adapters::describe_until(&spec, Build::Standard, &dataset, |_| {
            ControlFlow::<()>::Continue(())
        })
        .unwrap_or_else(|e| panic!("{case}: {e:?}"));
        let direct =
            stats_desc::describe(&dataset, &params).unwrap_or_else(|e| panic!("{case}: {e:?}"));
        assert_eq!(bytes(&wrapped), bytes(&direct), "{case}: result");
    }
    println!("R golden cases compared: {}", cases.len());
}

fn bytes<T: serde::Serialize>(v: &T) -> Vec<u8> {
    serde_json::to_vec(v).expect("serialises")
}
